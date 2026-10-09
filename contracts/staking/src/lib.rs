#![no_std]
//! Staking: examiners lock stake to claim an assignment. Stake is slashed for no-shows, plagiarism, or reviews the community rejects.

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, BytesN, Env};

/// Maximum basis points (100%)
const MAX_BPS: u32 = 10_000;

/// Configurable maximum slash percentage (50% = 5000 bps)
/// See ADR 0004 for governance parameters
const MAX_SLASH_BPS: u32 = 5_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyStaked = 1,
    NotStaked = 2,
    StillLocked = 3,
    InvalidBps = 4,
    SlashExceedsMax = 5,
    Unauthorized = 6,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StakeRecord {
    pub examiner: Address,
    pub claim_id: BytesN<32>,
    pub amount: i128,
    pub locked: bool,
}

#[contracttype]
enum DataKey {
    Stake(Address, BytesN<32>),
    /// Address authorized to call slash (e.g., cross-exam contract or governance)
    SlashAuthority,
}

#[contract]
pub struct Staking;

#[contractimpl]
impl Staking {
    /// Interface version. Bump when the public ABI changes.
    pub fn version(_env: Env) -> u32 {
        1
    }

    /// Initialize the contract with the slash authority address.
    /// This should be called once during deployment.
    pub fn initialize(env: Env, slash_authority: Address) {
        // In production, check if already initialized
        env.storage()
            .instance()
            .set(&DataKey::SlashAuthority, &slash_authority);
    }

    /// Lock stake to a claim. Examiner must authorize this action.
    /// Stake is held until the review is complete and accepted.
    pub fn stake(
        env: Env,
        examiner: Address,
        claim_id: BytesN<32>,
        amount: i128,
    ) -> Result<(), Error> {
        examiner.require_auth();

        let key = DataKey::Stake(examiner.clone(), claim_id.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::AlreadyStaked);
        }

        let record = StakeRecord {
            examiner,
            claim_id,
            amount,
            locked: true,
        };

        env.storage().persistent().set(&key, &record);
        Ok(())
    }

    /// Unlock and return stake after review completion.
    /// Only the examiner may unlock their own stake, and only if not currently locked.
    pub fn unlock(env: Env, examiner: Address, claim_id: BytesN<32>) -> Result<(), Error> {
        examiner.require_auth();

        let key = DataKey::Stake(examiner.clone(), claim_id.clone());
        let record: StakeRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotStaked)?;

        if record.locked {
            return Err(Error::StillLocked);
        }

        env.storage().persistent().remove(&key);
        Ok(())
    }

    /// Mark stake as unlockable after review is accepted or time limit expires.
    /// This is a restricted function. See ADR 0004.
    pub fn mark_unlockable(env: Env, examiner: Address, claim_id: BytesN<32>) -> Result<(), Error> {
        Self::require_slash_authority(&env)?;

        let key = DataKey::Stake(examiner.clone(), claim_id.clone());
        let mut record: StakeRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotStaked)?;

        record.locked = false;
        env.storage().persistent().set(&key, &record);
        Ok(())
    }

    /// Slash a percentage of staked amount for rejected reviews or no-shows.
    /// Restricted to the configured slash authority (see ADR 0004).
    ///
    /// # Arguments
    /// * `examiner` - The examiner whose stake will be slashed
    /// * `claim_id` - The claim assignment
    /// * `bps` - Basis points to slash (1 bps = 0.01%, max 10000 = 100%)
    ///
    /// # Errors
    /// * `InvalidBps` - if bps > 10000
    /// * `SlashExceedsMax` - if bps > MAX_SLASH_BPS (governance parameter)
    /// * `Unauthorized` - if caller is not the slash authority
    /// * `NotStaked` - if no stake exists
    pub fn slash(
        env: Env,
        examiner: Address,
        claim_id: BytesN<32>,
        bps: u32,
    ) -> Result<i128, Error> {
        Self::require_slash_authority(&env)?;

        if bps > MAX_BPS {
            return Err(Error::InvalidBps);
        }

        if bps > MAX_SLASH_BPS {
            return Err(Error::SlashExceedsMax);
        }

        let key = DataKey::Stake(examiner.clone(), claim_id.clone());
        let mut record: StakeRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotStaked)?;

        let slash_amount = (record.amount * (bps as i128)) / (MAX_BPS as i128);
        record.amount -= slash_amount;
        env.storage().persistent().set(&key, &record);

        Ok(slash_amount)
    }

    /// Get stake information for an examiner on a specific claim.
    pub fn get_stake(
        env: Env,
        examiner: Address,
        claim_id: BytesN<32>,
    ) -> Result<StakeRecord, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Stake(examiner, claim_id))
            .ok_or(Error::NotStaked)
    }

    /// Get the configured maximum slash basis points.
    pub fn max_slash_bps(_env: Env) -> u32 {
        MAX_SLASH_BPS
    }

    // Internal helper to check slash authority
    fn require_slash_authority(env: &Env) -> Result<(), Error> {
        let authority: Address = env
            .storage()
            .instance()
            .get(&DataKey::SlashAuthority)
            .ok_or(Error::Unauthorized)?;
        authority.require_auth();
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup() -> (Env, Address, StakingClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let authority = Address::generate(&env);
        let id = env.register(Staking, ());
        let client = StakingClient::new(&env, &id);
        client.initialize(&authority);
        (env, authority, client)
    }

    #[test]
    fn reports_version() {
        let (_, _, client) = setup();
        assert_eq!(client.version(), 1);
    }

    #[test]
    fn stake_and_unlock() {
        let (env, _, client) = setup();
        let examiner = Address::generate(&env);
        let claim_id = BytesN::from_array(&env, &[1u8; 32]);
        let amount = 1000i128;

        client.stake(&examiner, &claim_id, &amount);

        let record = client.get_stake(&examiner, &claim_id);
        assert_eq!(record.amount, amount);
        assert_eq!(record.locked, true);

        // Mark as unlockable (would be done by authority after review accepted)
        client.mark_unlockable(&examiner, &claim_id);

        let record = client.get_stake(&examiner, &claim_id);
        assert_eq!(record.locked, false);

        // Now can unlock
        client.unlock(&examiner, &claim_id);

        // Stake should be removed
        assert_eq!(
            client.try_get_stake(&examiner, &claim_id),
            Err(Ok(Error::NotStaked))
        );
    }

    #[test]
    fn double_stake_rejected() {
        let (env, _, client) = setup();
        let examiner = Address::generate(&env);
        let claim_id = BytesN::from_array(&env, &[2u8; 32]);

        client.stake(&examiner, &claim_id, &1000);
        let result = client.try_stake(&examiner, &claim_id, &2000);
        assert_eq!(result, Err(Ok(Error::AlreadyStaked)));
    }

    #[test]
    fn unlock_while_locked_rejected() {
        let (env, _, client) = setup();
        let examiner = Address::generate(&env);
        let claim_id = BytesN::from_array(&env, &[3u8; 32]);

        client.stake(&examiner, &claim_id, &1000);

        // Try to unlock while still locked
        let result = client.try_unlock(&examiner, &claim_id);
        assert_eq!(result, Err(Ok(Error::StillLocked)));
    }

    #[test]
    fn slash_reduces_stake() {
        let (env, _, client) = setup();
        let examiner = Address::generate(&env);
        let claim_id = BytesN::from_array(&env, &[4u8; 32]);
        let amount = 10_000i128;

        client.stake(&examiner, &claim_id, &amount);

        // Slash 20% (2000 bps)
        let slashed = client.slash(&examiner, &claim_id, &2000);
        assert_eq!(slashed, 2_000i128);

        let record = client.get_stake(&examiner, &claim_id);
        assert_eq!(record.amount, 8_000i128);
    }

    #[test]
    fn slash_bps_capped_at_10000() {
        let (env, _, client) = setup();
        let examiner = Address::generate(&env);
        let claim_id = BytesN::from_array(&env, &[5u8; 32]);

        client.stake(&examiner, &claim_id, &10_000);

        let result = client.try_slash(&examiner, &claim_id, &10_001);
        assert_eq!(result, Err(Ok(Error::InvalidBps)));
    }

    #[test]
    fn slash_exceeds_max_configurable() {
        let (env, _, client) = setup();
        let examiner = Address::generate(&env);
        let claim_id = BytesN::from_array(&env, &[6u8; 32]);

        client.stake(&examiner, &claim_id, &10_000);

        // MAX_SLASH_BPS is 5000 (50%)
        let result = client.try_slash(&examiner, &claim_id, &5_001);
        assert_eq!(result, Err(Ok(Error::SlashExceedsMax)));
    }

    #[test]
    fn slash_at_max_allowed() {
        let (env, _, client) = setup();
        let examiner = Address::generate(&env);
        let claim_id = BytesN::from_array(&env, &[7u8; 32]);
        let amount = 10_000i128;

        client.stake(&examiner, &claim_id, &amount);

        // Slash exactly at MAX_SLASH_BPS (5000 = 50%)
        let slashed = client.slash(&examiner, &claim_id, &5_000);
        assert_eq!(slashed, 5_000i128);

        let record = client.get_stake(&examiner, &claim_id);
        assert_eq!(record.amount, 5_000i128);
    }

    #[test]
    fn get_max_slash_bps() {
        let (_, _, client) = setup();
        assert_eq!(client.max_slash_bps(), 5_000);
    }

    #[test]
    fn not_staked_error() {
        let (env, _, client) = setup();
        let examiner = Address::generate(&env);
        let claim_id = BytesN::from_array(&env, &[8u8; 32]);

        assert_eq!(
            client.try_get_stake(&examiner, &claim_id),
            Err(Ok(Error::NotStaked))
        );
    }
}
