#![no_std]
//! Inquiry escrow: claim bounties deposited as Stellar Asset Contract tokens.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, BytesN, Env,
};

const TTL_THRESHOLD: u32 = 518_400;
const TTL_BUMP: u32 = 518_400;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    AlreadyFunded = 3,
    NotFound = 4,
    InvalidAmount = 5,
    InsufficientBalance = 6,
    AlreadyRefunded = 7,
}

#[contracttype]
#[derive(Clone)]
struct Config {
    authority: Address,
}

#[contracttype]
#[derive(Clone)]
struct Bounty {
    funder: Address,
    token: Address,
    remaining: i128,
    refunded: bool,
}

#[contracttype]
enum DataKey {
    Config,
    Bounty(BytesN<32>),
}

#[contract]
pub struct InquiryEscrow;

#[contractimpl]
impl InquiryEscrow {
    /// Interface version. Bump when the public ABI changes.
    pub fn version(_env: Env) -> u32 {
        1
    }

    /// Set the immutable settlement authority. Call during deployment.
    pub fn initialize(env: Env, authority: Address) -> Result<(), Error> {
        authority.require_auth();
        let storage = env.storage().persistent();
        if storage.has(&DataKey::Config) {
            return Err(Error::AlreadyInitialized);
        }
        let key = DataKey::Config;
        storage.set(&key, &Config { authority });
        storage.extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        Ok(())
    }

    /// Deposit a claim's bounty. A claim may be funded once with a single token.
    pub fn deposit(
        env: Env,
        claim_id: BytesN<32>,
        funder: Address,
        token_address: Address,
        amount: i128,
    ) -> Result<(), Error> {
        funder.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        get_config(&env)?;
        let storage = env.storage().persistent();
        let key = DataKey::Bounty(claim_id);
        if storage.has(&key) {
            return Err(Error::AlreadyFunded);
        }

        token::Client::new(&env, &token_address).transfer(
            &funder,
            env.current_contract_address(),
            &amount,
        );

        storage.set(
            &key,
            &Bounty {
                funder,
                token: token_address,
                remaining: amount,
                refunded: false,
            },
        );
        storage.extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        Ok(())
    }

    /// Release bounty funds to an examiner after settlement authorization.
    pub fn release(
        env: Env,
        claim_id: BytesN<32>,
        examiner: Address,
        amount: i128,
    ) -> Result<(), Error> {
        let config = get_config(&env)?;
        config.authority.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let key = DataKey::Bounty(claim_id);
        let mut bounty: Bounty = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotFound)?;
        if bounty.refunded {
            return Err(Error::AlreadyRefunded);
        }
        if amount > bounty.remaining {
            return Err(Error::InsufficientBalance);
        }

        token::Client::new(&env, &bounty.token).transfer(
            &env.current_contract_address(),
            &examiner,
            &amount,
        );
        bounty.remaining -= amount;
        env.storage().persistent().set(&key, &bounty);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        Ok(())
    }

    /// Return the unspent remainder to the original funder.
    pub fn refund_remaining(env: Env, claim_id: BytesN<32>) -> Result<i128, Error> {
        let config = get_config(&env)?;
        config.authority.require_auth();
        let key = DataKey::Bounty(claim_id);
        let mut bounty: Bounty = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotFound)?;
        if bounty.refunded {
            return Err(Error::AlreadyRefunded);
        }

        let amount = bounty.remaining;
        if amount > 0 {
            token::Client::new(&env, &bounty.token).transfer(
                &env.current_contract_address(),
                &bounty.funder,
                &amount,
            );
        }
        bounty.remaining = 0;
        bounty.refunded = true;
        env.storage().persistent().set(&key, &bounty);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        Ok(amount)
    }

    pub fn remaining(env: Env, claim_id: BytesN<32>) -> Result<i128, Error> {
        get_config(&env)?;
        let key = DataKey::Bounty(claim_id);
        let remaining = env
            .storage()
            .persistent()
            .get::<_, Bounty>(&key)
            .map(|bounty| bounty.remaining)
            .ok_or(Error::NotFound)?;
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        Ok(remaining)
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
    use soroban_sdk::{
        testutils::{storage::Persistent as _, Address as _},
        token::StellarAssetClient,
    };

    fn setup() -> (
        Env,
        InquiryEscrowClient<'static>,
        Address,
        Address,
        Address,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();
        let authority = Address::generate(&env);
        let funder = Address::generate(&env);
        let examiner = Address::generate(&env);
        let contract_id = env.register(InquiryEscrow, ());
        let client = InquiryEscrowClient::new(&env, &contract_id);
        client.initialize(&authority);
        (env, client, authority, funder, examiner, contract_id)
    }

    #[test]
    fn releases_and_refunds_remaining_bounty() {
        let (env, client, authority, funder, examiner, _) = setup();
        assert_eq!(client.version(), 1);
        let token = env.register_stellar_asset_contract_v2(authority).address();
        let claim_id = BytesN::from_array(&env, &[1; 32]);
        StellarAssetClient::new(&env, &token).mint(&funder, &100);
        client.deposit(&claim_id, &funder, &token, &100);
        env.mock_auths(&[]);
        assert!(client.try_refund_remaining(&claim_id).is_err());
        env.mock_all_auths();
        client.release(&claim_id, &examiner, &40);
        assert_eq!(StellarAssetClient::new(&env, &token).balance(&examiner), 40);
        assert_eq!(client.remaining(&claim_id), 60);
        assert_eq!(client.refund_remaining(&claim_id), 60);
        assert_eq!(StellarAssetClient::new(&env, &token).balance(&funder), 60);
        assert_eq!(
            client.try_refund_remaining(&claim_id),
            Err(Ok(Error::AlreadyRefunded))
        );
    }

    #[test]
    fn rejects_over_release() {
        let (env, client, authority, funder, examiner, _) = setup();
        let token = env.register_stellar_asset_contract_v2(authority).address();
        let claim_id = BytesN::from_array(&env, &[2; 32]);
        StellarAssetClient::new(&env, &token).mint(&funder, &10);
        client.deposit(&claim_id, &funder, &token, &10);
        assert_eq!(
            client.try_release(&claim_id, &examiner, &11),
            Err(Ok(Error::InsufficientBalance))
        );
    }

    #[test]
    fn rejects_release_without_authority() {
        let (env, client, authority, funder, examiner, _) = setup();
        let token = env
            .register_stellar_asset_contract_v2(authority.clone())
            .address();
        let claim_id = BytesN::from_array(&env, &[3; 32]);
        StellarAssetClient::new(&env, &token).mint(&funder, &10);
        client.deposit(&claim_id, &funder, &token, &10);

        env.mock_auths(&[]);
        let result = client.try_release(&claim_id, &examiner, &1);
        assert!(result.is_err());
    }

    #[test]
    fn deposit_extends_persistent_entry_ttl() {
        let (env, client, authority, funder, _, contract_id) = setup();
        let token = env.register_stellar_asset_contract_v2(authority).address();
        let claim_id = BytesN::from_array(&env, &[4; 32]);
        StellarAssetClient::new(&env, &token).mint(&funder, &10);
        client.deposit(&claim_id, &funder, &token, &10);

        let ttl = env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .get_ttl(&DataKey::Bounty(claim_id.clone()))
        });
        assert!(ttl >= env.ledger().sequence() + TTL_BUMP);
    }
}
