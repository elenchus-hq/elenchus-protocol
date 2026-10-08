#![no_std]
//! Cross-examination: allowlisted, reputation-weighted commit-reveal voting.

use reputation::ReputationClient;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, xdr::ToXdr, Address, BytesN, Env, Symbol,
    Vec,
};

const MAX_VOTE_WEIGHT: i128 = 1_000_000;
const GROUP_WEIGHT_CAP: i128 = 1_000;
const MAX_VOTERS_PER_REVIEW: u32 = 100;
const MAX_QUALITY: u32 = 100;
const TTL_THRESHOLD: u32 = 518_400;
const TTL_BUMP: u32 = 518_400;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    AlreadyExists = 3,
    NotFound = 4,
    InvalidPeriod = 5,
    InvalidQuality = 6,
    InvalidCommitment = 7,
    NotEligible = 8,
    AlreadyVoted = 9,
    TooManyVoters = 10,
    CommitPeriodClosed = 11,
    RevealPeriodNotOpen = 12,
    RevealPeriodClosed = 13,
    AlreadyRevealed = 14,
    ArithmeticOverflow = 15,
}

#[contracttype]
#[derive(Clone)]
struct Config {
    admin: Address,
    reputation: Address,
    commit_period: u64,
    reveal_period: u64,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoterGroup {
    Author,
    Examiner,
    Editor,
}

#[contracttype]
#[derive(Clone)]
struct Review {
    author: Address,
    field: Symbol,
    commit_end: u64,
    reveal_end: u64,
    voters: Vec<Address>,
}

#[contracttype]
#[derive(Clone)]
struct VoteCommit {
    hash: BytesN<32>,
    group: VoterGroup,
    weight: i128,
    revealed_quality: Option<u32>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tally {
    pub weighted_quality: i128,
    pub total_weight: i128,
    pub revealed_votes: u32,
}

#[derive(Clone, Copy, Default)]
struct GroupTotals {
    weight: i128,
    weighted_quality: i128,
}

impl VoterGroup {
    fn index(self) -> usize {
        match self {
            Self::Author => 0,
            Self::Examiner => 1,
            Self::Editor => 2,
        }
    }
}

#[contracttype]
enum DataKey {
    Config,
    Review(BytesN<32>),
    VoterGroup(Address),
    Vote(BytesN<32>, Address),
}

#[contract]
pub struct CrossExam;

#[contractimpl]
impl CrossExam {
    /// Interface version. Bump when the public ABI changes.
    pub fn version(_env: Env) -> u32 {
        1
    }

    /// Configure the authority, reputation contract, and fixed voting windows.
    pub fn initialize(
        env: Env,
        admin: Address,
        reputation: Address,
        commit_period: u64,
        reveal_period: u64,
    ) -> Result<(), Error> {
        admin.require_auth();
        if commit_period == 0 || reveal_period == 0 {
            return Err(Error::InvalidPeriod);
        }
        let storage = env.storage().persistent();
        let key = DataKey::Config;
        if storage.has(&key) {
            return Err(Error::AlreadyInitialized);
        }
        storage.set(
            &key,
            &Config {
                admin,
                reputation,
                commit_period,
                reveal_period,
            },
        );
        storage.extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        Ok(())
    }

    /// Allow an address to vote in one of the explicitly supported groups.
    pub fn set_voter_group(env: Env, voter: Address, group: VoterGroup) -> Result<(), Error> {
        let config = get_config(&env)?;
        config.admin.require_auth();
        let key = DataKey::VoterGroup(voter);
        env.storage().persistent().set(&key, &group);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        Ok(())
    }

    /// Open a review vote with its trusted author and reputation field.
    pub fn register_review(
        env: Env,
        review_id: BytesN<32>,
        author: Address,
        field: Symbol,
    ) -> Result<(), Error> {
        let config = get_config(&env)?;
        config.admin.require_auth();
        let key = DataKey::Review(review_id);
        if env.storage().persistent().has(&key) {
            return Err(Error::AlreadyExists);
        }
        let now = env.ledger().timestamp();
        let commit_end = now
            .checked_add(config.commit_period)
            .ok_or(Error::ArithmeticOverflow)?;
        let reveal_end = commit_end
            .checked_add(config.reveal_period)
            .ok_or(Error::ArithmeticOverflow)?;
        env.storage().persistent().set(
            &key,
            &Review {
                author,
                field,
                commit_end,
                reveal_end,
                voters: Vec::new(&env),
            },
        );
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        Ok(())
    }

    /// Commit to a quality score before the review's commit deadline.
    pub fn commit(
        env: Env,
        voter: Address,
        review_id: BytesN<32>,
        hash: BytesN<32>,
    ) -> Result<(), Error> {
        voter.require_auth();
        let config = get_config(&env)?;
        let review_key = DataKey::Review(review_id.clone());
        let mut review: Review = env
            .storage()
            .persistent()
            .get(&review_key)
            .ok_or(Error::NotFound)?;
        if env.ledger().timestamp() > review.commit_end {
            return Err(Error::CommitPeriodClosed);
        }
        if voter == review.author {
            return Err(Error::NotEligible);
        }
        let voter_group: VoterGroup = env
            .storage()
            .persistent()
            .get(&DataKey::VoterGroup(voter.clone()))
            .ok_or(Error::NotEligible)?;
        env.storage().persistent().extend_ttl(
            &DataKey::VoterGroup(voter.clone()),
            TTL_THRESHOLD,
            TTL_BUMP,
        );
        let vote_key = DataKey::Vote(review_id, voter.clone());
        if env.storage().persistent().has(&vote_key) {
            return Err(Error::AlreadyVoted);
        }
        if review.voters.len() >= MAX_VOTERS_PER_REVIEW {
            return Err(Error::TooManyVoters);
        }

        let reputation =
            ReputationClient::new(&env, &config.reputation).score(&voter, &review.field);
        let weight = reputation.clamp(0, MAX_VOTE_WEIGHT);
        if weight == 0 {
            return Err(Error::NotEligible);
        }
        review.voters.push_back(voter.clone());
        env.storage().persistent().set(
            &vote_key,
            &VoteCommit {
                hash,
                group: voter_group,
                weight,
                revealed_quality: None,
            },
        );
        env.storage().persistent().set(&review_key, &review);
        env.storage()
            .persistent()
            .extend_ttl(&vote_key, TTL_THRESHOLD, TTL_BUMP);
        env.storage()
            .persistent()
            .extend_ttl(&review_key, TTL_THRESHOLD, TTL_BUMP);
        Ok(())
    }

    /// Reveal a previously committed quality score and salt.
    pub fn reveal(
        env: Env,
        voter: Address,
        review_id: BytesN<32>,
        quality: u32,
        salt: BytesN<32>,
    ) -> Result<(), Error> {
        voter.require_auth();
        if quality > MAX_QUALITY {
            return Err(Error::InvalidQuality);
        }
        let review_key = DataKey::Review(review_id.clone());
        let review: Review = env
            .storage()
            .persistent()
            .get(&review_key)
            .ok_or(Error::NotFound)?;
        let now = env.ledger().timestamp();
        if now <= review.commit_end {
            return Err(Error::RevealPeriodNotOpen);
        }
        if now > review.reveal_end {
            return Err(Error::RevealPeriodClosed);
        }
        let vote_key = DataKey::Vote(review_id.clone(), voter.clone());
        let mut vote: VoteCommit = env
            .storage()
            .persistent()
            .get(&vote_key)
            .ok_or(Error::NotFound)?;
        if vote.revealed_quality.is_some() {
            return Err(Error::AlreadyRevealed);
        }
        if vote.hash != commitment_hash(&env, &review_id, &voter, quality, &salt) {
            return Err(Error::InvalidCommitment);
        }
        vote.revealed_quality = Some(quality);
        env.storage().persistent().set(&vote_key, &vote);
        env.storage()
            .persistent()
            .extend_ttl(&vote_key, TTL_THRESHOLD, TTL_BUMP);
        env.storage()
            .persistent()
            .extend_ttl(&review_key, TTL_THRESHOLD, TTL_BUMP);
        Ok(())
    }

    /// Tally revealed votes once the reveal window has closed.
    pub fn tally(env: Env, review_id: BytesN<32>) -> Result<Tally, Error> {
        let review: Review = env
            .storage()
            .persistent()
            .get(&DataKey::Review(review_id.clone()))
            .ok_or(Error::NotFound)?;
        if env.ledger().timestamp() <= review.reveal_end {
            return Err(Error::RevealPeriodNotOpen);
        }
        env.storage().persistent().extend_ttl(
            &DataKey::Review(review_id.clone()),
            TTL_THRESHOLD,
            TTL_BUMP,
        );

        let mut groups = [GroupTotals::default(); 3];
        let mut revealed_votes = 0;
        for voter in review.voters.iter() {
            let vote: VoteCommit = env
                .storage()
                .persistent()
                .get(&DataKey::Vote(review_id.clone(), voter))
                .ok_or(Error::NotFound)?;
            if let Some(quality) = vote.revealed_quality {
                let group = &mut groups[vote.group.index()];
                group.weight = group
                    .weight
                    .checked_add(vote.weight)
                    .ok_or(Error::ArithmeticOverflow)?;
                group.weighted_quality = group
                    .weighted_quality
                    .checked_add(
                        vote.weight
                            .checked_mul(i128::from(quality))
                            .ok_or(Error::ArithmeticOverflow)?,
                    )
                    .ok_or(Error::ArithmeticOverflow)?;
                revealed_votes += 1;
            }
        }

        let mut tally = Tally {
            weighted_quality: 0,
            total_weight: 0,
            revealed_votes,
        };
        for group in groups {
            if group.weight > 0 {
                let capped_weight = group.weight.min(GROUP_WEIGHT_CAP);
                let capped_quality = group
                    .weighted_quality
                    .checked_mul(capped_weight)
                    .ok_or(Error::ArithmeticOverflow)?
                    .checked_div(group.weight)
                    .ok_or(Error::ArithmeticOverflow)?;
                tally.weighted_quality = tally
                    .weighted_quality
                    .checked_add(capped_quality)
                    .ok_or(Error::ArithmeticOverflow)?;
                tally.total_weight = tally
                    .total_weight
                    .checked_add(capped_weight)
                    .ok_or(Error::ArithmeticOverflow)?;
            }
        }
        Ok(tally)
    }
}

fn get_config(env: &Env) -> Result<Config, Error> {
    let storage = env.storage().persistent();
    let config = storage.get(&DataKey::Config).ok_or(Error::NotInitialized)?;
    storage.extend_ttl(&DataKey::Config, TTL_THRESHOLD, TTL_BUMP);
    Ok(config)
}

fn commitment_hash(
    env: &Env,
    review_id: &BytesN<32>,
    voter: &Address,
    quality: u32,
    salt: &BytesN<32>,
) -> BytesN<32> {
    let preimage = (review_id.clone(), voter.clone(), quality, salt.clone()).to_xdr(env);
    env.crypto().sha256(&preimage).into()
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Ledger},
    };

    struct Fixture {
        env: Env,
        client: CrossExamClient<'static>,
        author: Address,
        voters: [Address; 2],
        review_id: BytesN<32>,
    }

    fn setup() -> Fixture {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let author = Address::generate(&env);
        let voters = [Address::generate(&env), Address::generate(&env)];
        let reputation_id = env.register(reputation::Reputation, ());
        let reputation = ReputationClient::new(&env, &reputation_id);
        reputation.initialize(&admin);
        reputation.record_outcome(&voters[0], &symbol_short!("biology"), &1_500);
        reputation.record_outcome(&voters[1], &symbol_short!("biology"), &500);

        let id = env.register(CrossExam, ());
        let client = CrossExamClient::new(&env, &id);
        client.initialize(&admin, &reputation_id, &10, &10);
        assert_eq!(client.version(), 1);
        client.set_voter_group(&voters[0], &VoterGroup::Examiner);
        client.set_voter_group(&voters[1], &VoterGroup::Examiner);
        let review_id = BytesN::from_array(&env, &[1; 32]);
        client.register_review(&review_id, &author, &symbol_short!("biology"));
        Fixture {
            env,
            client,
            author,
            voters,
            review_id,
        }
    }

    fn commit_vote(fixture: &Fixture, voter: &Address, quality: u32, salt: &BytesN<32>) {
        let hash = commitment_hash(&fixture.env, &fixture.review_id, voter, quality, salt);
        fixture.client.commit(voter, &fixture.review_id, &hash);
    }

    #[test]
    fn rejects_a_second_commit_for_the_same_review() {
        let fixture = setup();
        let salt = BytesN::from_array(&fixture.env, &[2; 32]);
        commit_vote(&fixture, &fixture.voters[0], 80, &salt);
        let hash = commitment_hash(
            &fixture.env,
            &fixture.review_id,
            &fixture.voters[0],
            80,
            &salt,
        );
        assert_eq!(
            fixture
                .client
                .try_commit(&fixture.voters[0], &fixture.review_id, &hash),
            Err(Ok(Error::AlreadyVoted))
        );
    }

    #[test]
    fn rejects_a_reveal_after_the_deadline() {
        let fixture = setup();
        let salt = BytesN::from_array(&fixture.env, &[3; 32]);
        commit_vote(&fixture, &fixture.voters[0], 80, &salt);
        fixture.env.ledger().set_timestamp(21);
        assert_eq!(
            fixture
                .client
                .try_reveal(&fixture.voters[0], &fixture.review_id, &80, &salt),
            Err(Ok(Error::RevealPeriodClosed))
        );
    }

    #[test]
    fn caps_aggregate_weight_per_voter_group() {
        let fixture = setup();
        let salt_a = BytesN::from_array(&fixture.env, &[4; 32]);
        let salt_b = BytesN::from_array(&fixture.env, &[5; 32]);
        commit_vote(&fixture, &fixture.voters[0], 100, &salt_a);
        commit_vote(&fixture, &fixture.voters[1], 0, &salt_b);
        fixture.env.ledger().set_timestamp(11);
        fixture
            .client
            .reveal(&fixture.voters[0], &fixture.review_id, &100, &salt_a);
        fixture
            .client
            .reveal(&fixture.voters[1], &fixture.review_id, &0, &salt_b);
        fixture.env.ledger().set_timestamp(21);

        let tally = fixture.client.tally(&fixture.review_id);
        assert_eq!(tally.total_weight, GROUP_WEIGHT_CAP);
        assert_eq!(tally.weighted_quality, 75_000);
        assert_eq!(tally.revealed_votes, 2);
    }

    #[test]
    fn excludes_the_review_author_from_the_allowlisted_electorate() {
        let fixture = setup();
        let salt = BytesN::from_array(&fixture.env, &[6; 32]);
        let hash = commitment_hash(&fixture.env, &fixture.review_id, &fixture.author, 50, &salt);
        fixture
            .client
            .set_voter_group(&fixture.author, &VoterGroup::Author);
        assert_eq!(
            fixture
                .client
                .try_commit(&fixture.author, &fixture.review_id, &hash),
            Err(Ok(Error::NotEligible))
        );
    }
}
