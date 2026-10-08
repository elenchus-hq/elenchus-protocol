#![no_std]
//! Reputation: non-transferable, field-scoped examiner score.

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol};

const TTL_THRESHOLD: u32 = 518_400;
const TTL_BUMP: u32 = 518_400;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    ArithmeticOverflow = 3,
}

#[contracttype]
#[derive(Clone)]
struct Config {
    authority: Address,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Config,
    Score(Address, Symbol),
}

#[contract]
pub struct Reputation;

#[contractimpl]
impl Reputation {
    /// Interface version. Bump when the public ABI changes.
    pub fn version(_env: Env) -> u32 {
        1
    }

    /// Set the immutable outcome-recording authority. Call during deployment.
    pub fn initialize(env: Env, authority: Address) -> Result<(), Error> {
        authority.require_auth();
        let key = DataKey::Config;
        let storage = env.storage().persistent();
        if storage.has(&key) {
            return Err(Error::AlreadyInitialized);
        }
        storage.set(&key, &Config { authority });
        storage.extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        Ok(())
    }

    /// Return an examiner's score in a case-sensitive field.
    pub fn score(env: Env, examiner: Address, field: Symbol) -> i128 {
        let key = DataKey::Score(examiner, field);
        let storage = env.storage().persistent();
        match storage.get(&key) {
            Some(score) => {
                storage.extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
                score
            }
            None => 0,
        }
    }

    /// Record a quality outcome. Only the configured settlement authority may update scores.
    pub fn record_outcome(
        env: Env,
        examiner: Address,
        field: Symbol,
        delta: i128,
    ) -> Result<i128, Error> {
        let storage = env.storage().persistent();
        let config: Config = storage.get(&DataKey::Config).ok_or(Error::NotInitialized)?;
        config.authority.require_auth();
        storage.extend_ttl(&DataKey::Config, TTL_THRESHOLD, TTL_BUMP);

        let key = DataKey::Score(examiner, field);
        let current: i128 = storage.get(&key).unwrap_or(0);
        let updated = current
            .checked_add(delta)
            .ok_or(Error::ArithmeticOverflow)?;
        env.storage().persistent().set(&key, &updated);
        env.storage()
            .persistent()
            .extend_ttl(&key, TTL_THRESHOLD, TTL_BUMP);
        Ok(updated)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn records_independent_field_scoped_scores() {
        let env = Env::default();
        env.mock_all_auths();
        let authority = Address::generate(&env);
        let examiner = Address::generate(&env);
        let id = env.register(Reputation, ());
        let client = ReputationClient::new(&env, &id);
        client.initialize(&authority);
        assert_eq!(client.version(), 1);
        let physics = soroban_sdk::symbol_short!("physics");
        let biology = soroban_sdk::symbol_short!("biology");

        assert_eq!(client.score(&examiner, &physics), 0);
        assert_eq!(client.record_outcome(&examiner, &physics, &5), 5);
        assert_eq!(client.record_outcome(&examiner, &physics, &-2), 3);
        assert_eq!(client.record_outcome(&examiner, &biology, &4), 4);
        assert_eq!(client.score(&examiner, &physics), 3);
        assert_eq!(client.score(&examiner, &biology), 4);
    }

    #[test]
    fn third_party_cannot_set_an_examiner_score() {
        let env = Env::default();
        let authority = Address::generate(&env);
        let examiner = Address::generate(&env);
        let id = env.register(Reputation, ());
        let client = ReputationClient::new(&env, &id);
        env.mock_all_auths();
        client.initialize(&authority);

        env.mock_auths(&[]);
        let field = soroban_sdk::symbol_short!("physics");
        assert!(client.try_record_outcome(&examiner, &field, &100).is_err());
        env.mock_all_auths();
        assert_eq!(client.score(&examiner, &field), 0);
    }
}
