#![no_std]
//! Claim registry: the on-chain anchor for a manuscript.
//!
//! The manuscript lives off-chain (IPFS). Only its SHA-256 hash (the claim id),
//! its CID, the author, and a timestamp are stored here.

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, Address, BytesN, Env, String,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyRegistered = 1,
    NotFound = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claim {
    pub author: Address,
    pub content_hash: BytesN<32>,
    pub cid: String,
    pub registered_at: u64,
}

#[contracttype]
enum DataKey {
    Claim(BytesN<32>),
}

#[contract]
pub struct ClaimRegistry;

#[contractimpl]
impl ClaimRegistry {
    /// Register a manuscript. Requires the author's authorization.
    /// A given content hash can be registered once.
    pub fn register_claim(
        env: Env,
        author: Address,
        content_hash: BytesN<32>,
        cid: String,
    ) -> Result<(), Error> {
        author.require_auth();

        let key = DataKey::Claim(content_hash.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::AlreadyRegistered);
        }

        let claim = Claim {
            author,
            content_hash,
            cid,
            registered_at: env.ledger().timestamp(),
        };
        env.storage().persistent().set(&key, &claim);
        // TODO(good-first-issue): emit a `claim_registered` event and extend the entry TTL.
        Ok(())
    }

    pub fn get_claim(env: Env, content_hash: BytesN<32>) -> Result<Claim, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Claim(content_hash))
            .ok_or(Error::NotFound)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup() -> (Env, ClaimRegistryClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(ClaimRegistry, ());
        let client = ClaimRegistryClient::new(&env, &id);
        (env, client)
    }

    #[test]
    fn register_then_fetch() {
        let (env, client) = setup();
        let author = Address::generate(&env);
        let hash = BytesN::from_array(&env, &[1u8; 32]);
        let cid = String::from_str(&env, "bafybeigdyrzt5example");

        client.register_claim(&author, &hash, &cid);

        let claim = client.get_claim(&hash);
        assert_eq!(claim.author, author);
        assert_eq!(claim.cid, cid);
    }

    #[test]
    fn duplicate_hash_is_rejected() {
        let (env, client) = setup();
        let author = Address::generate(&env);
        let hash = BytesN::from_array(&env, &[2u8; 32]);
        let cid = String::from_str(&env, "bafybeigdyrzt5example");

        client.register_claim(&author, &hash, &cid);
        let again = client.try_register_claim(&author, &hash, &cid);
        assert_eq!(again, Err(Ok(Error::AlreadyRegistered)));
    }

    #[test]
    fn unknown_claim_is_not_found() {
        let (env, client) = setup();
        let hash = BytesN::from_array(&env, &[9u8; 32]);
        assert_eq!(client.try_get_claim(&hash), Err(Ok(Error::NotFound)));
    }
}
