#![no_std]
//! Staking: examiners lock SAC tokens to a claim assignment.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, BytesN, Env,
};

const BASIS_POINTS: u32 = 10_000;
const TTL_THRESHOLD: u32 = 518_400;
const TTL_BUMP: u32 = 518_400;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    AlreadyStaked = 3,
    NotStaked = 4,
    InvalidAmount = 5,
    InvalidMaximumSlash = 6,
    SlashAboveMaximum = 7,
    AlreadySlashed = 8,
    ArithmeticOverflow = 9,
}

#[contracttype]
#[derive(Clone)]
struct Config {
    authority: Address,
    token: Address,
    treasury: Address,
    max_slash_bps: u32,
}

#[contracttype]
#[derive(Clone)]
struct Stake {
    remaining: i128,
    slashed: bool,
}

#[contracttype]
enum DataKey {
    Config,
    Stake(Address, BytesN<32>),
}

#[contract]
pub struct Staking;

#[contractimpl]
impl Staking {
    /// Interface version. Bump when the public ABI changes.
    pub fn version(_env: Env) -> u32 {
        1
    }

    /// Configure the immutable token, treasury, and slash authority.
    pub fn initialize(
        env: Env,
        authority: Address,
        token_address: Address,
        treasury: Address,
        max_slash_bps: u32,
    ) -> Result<(), Error> {
        authority.require_auth();
        if max_slash_bps == 0 || max_slash_bps > BASIS_POINTS {
            return Err(Error::InvalidMaximumSlash);
        }

        let storage = env.storage().persistent();
        if storage.has(&DataKey::Config) {
            return Err(Error::AlreadyInitialized);
        }
        let key = DataKey::Config;
        storage.set(
            &key,
            &Config {
                authority,
                token: token_address,
                treasury,
                max_slash_bps,
            },
        );
        storage.extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        Ok(())
    }

    /// Lock tokens to an examiner's claim assignment.
    pub fn stake(
        env: Env,
        examiner: Address,
        claim_id: BytesN<32>,
        amount: i128,
    ) -> Result<(), Error> {
        examiner.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let config = get_config(&env)?;
        let key = DataKey::Stake(examiner.clone(), claim_id);
        if env.storage().persistent().has(&key) {
            return Err(Error::AlreadyStaked);
        }

        token::Client::new(&env, &config.token).transfer(
            &examiner,
            env.current_contract_address(),
            &amount,
        );
        env.storage().persistent().set(
            &key,
            &Stake {
                remaining: amount,
                slashed: false,
            },
        );
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        Ok(())
    }

    /// Slash a stake after the authorized settlement process.
    pub fn slash(
        env: Env,
        examiner: Address,
        claim_id: BytesN<32>,
        bps: u32,
    ) -> Result<i128, Error> {
        let config = get_config(&env)?;
        config.authority.require_auth();
        if bps > config.max_slash_bps || bps > BASIS_POINTS {
            return Err(Error::SlashAboveMaximum);
        }

        let key = DataKey::Stake(examiner, claim_id);
        let mut stake: Stake = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotStaked)?;
        if stake.slashed {
            return Err(Error::AlreadySlashed);
        }
        let amount = stake
            .remaining
            .checked_mul(i128::from(bps))
            .ok_or(Error::ArithmeticOverflow)?
            / i128::from(BASIS_POINTS);
        if amount > 0 {
            token::Client::new(&env, &config.token).transfer(
                &env.current_contract_address(),
                &config.treasury,
                &amount,
            );
        }
        stake.remaining -= amount;
        stake.slashed = true;
        env.storage().persistent().set(&key, &stake);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        Ok(amount)
    }

    /// Return the unslashed stake after the authorized settlement process.
    pub fn unlock(env: Env, examiner: Address, claim_id: BytesN<32>) -> Result<i128, Error> {
        let config = get_config(&env)?;
        config.authority.require_auth();

        let key = DataKey::Stake(examiner.clone(), claim_id);
        let stake: Stake = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotStaked)?;
        if stake.remaining > 0 {
            token::Client::new(&env, &config.token).transfer(
                &env.current_contract_address(),
                &examiner,
                &stake.remaining,
            );
        }
        env.storage().persistent().remove(&key);
        Ok(stake.remaining)
    }
}

fn get_config(env: &Env) -> Result<Config, Error> {
    let storage = env.storage().persistent();
    let config = storage.get(&DataKey::Config).ok_or(Error::NotInitialized)?;
    storage.extend_ttl(&DataKey::Config, TTL_THRESHOLD, TTL_BUMP);
    Ok(config)
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, token::StellarAssetClient};

    fn setup(max_slash_bps: u32) -> (Env, StakingClient<'static>, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let authority = Address::generate(&env);
        let examiner = Address::generate(&env);
        let treasury = Address::generate(&env);
        let issuer = Address::generate(&env);
        let token = env.register_stellar_asset_contract_v2(issuer);
        let token_address = token.address();
        let id = env.register(Staking, ());
        let client = StakingClient::new(&env, &id);
        client.initialize(&authority, &token_address, &treasury, &max_slash_bps);
        StellarAssetClient::new(&env, &token_address).mint(&examiner, &100);
        (env, client, examiner, treasury, token_address)
    }

    #[test]
    fn keeps_stake_locked_until_authorized_settlement() {
        let (env, client, examiner, _, _) = setup(5_000);
        assert_eq!(client.version(), 1);
        let claim_id = BytesN::from_array(&env, &[1; 32]);
        client.stake(&examiner, &claim_id, &50);
        assert_eq!(
            client.try_stake(&examiner, &claim_id, &10),
            Err(Ok(Error::AlreadyStaked))
        );

        env.mock_auths(&[]);
        assert!(client.try_unlock(&examiner, &claim_id).is_err());
        env.mock_all_auths();
        assert_eq!(client.unlock(&examiner, &claim_id), 50);
        assert_eq!(
            client.try_unlock(&examiner, &claim_id),
            Err(Ok(Error::NotStaked))
        );
    }

    #[test]
    fn applies_the_configured_slash_limit() {
        let (env, client, examiner, treasury, token) = setup(2_500);
        let claim_id = BytesN::from_array(&env, &[2; 32]);
        client.stake(&examiner, &claim_id, &100);
        env.mock_auths(&[]);
        assert!(client.try_slash(&examiner, &claim_id, &1).is_err());
        env.mock_all_auths();
        assert_eq!(
            client.try_slash(&examiner, &claim_id, &2_501),
            Err(Ok(Error::SlashAboveMaximum))
        );
        assert_eq!(client.slash(&examiner, &claim_id, &2_500), 25);
        assert_eq!(StellarAssetClient::new(&env, &token).balance(&treasury), 25);
        assert_eq!(client.unlock(&examiner, &claim_id), 75);
    }
}
