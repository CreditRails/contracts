#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, BytesN, Env, String};

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub struct Credential {
    pub wallet: Address,
    pub credential_id: String,
    pub score: u32,
    pub issued_ledger: u32,
    pub expires_ledger: u32,
    /// SHA-256 of the JWT string — full JWT is not stored on-chain
    pub jwt_hash: BytesN<32>,
    pub revoked: bool,
}

#[contracttype]
pub enum DataKey {
    Credential(String),    // credential_id → Credential
    WalletLatest(Address), // wallet → latest credential_id
    Admin,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct CredentialRegistry;

#[contractimpl]
impl CredentialRegistry {
    /// One-time setup.
    pub fn initialize(env: Env, admin: Address) {
        assert!(
            !env.storage().instance().has(&DataKey::Admin),
            "already initialized"
        );
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Admin-only: anchor a new credential on-chain after issuance.
    /// Stores the SHA-256 of the JWT so verifiers can confirm authenticity
    /// without exposing the full token.
    pub fn register(
        env: Env,
        wallet: Address,
        credential_id: String,
        score: u32,
        expires_ledger: u32,
        jwt_hash: BytesN<32>,
    ) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        assert!((300..=850).contains(&score), "score must be 300–850");
        assert!(
            expires_ledger > env.ledger().sequence(),
            "expiry must be in the future"
        );

        let cred = Credential {
            wallet: wallet.clone(),
            credential_id: credential_id.clone(),
            score,
            issued_ledger: env.ledger().sequence(),
            expires_ledger,
            jwt_hash,
            revoked: false,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Credential(credential_id.clone()), &cred);
        env.storage()
            .persistent()
            .set(&DataKey::WalletLatest(wallet), &credential_id);
    }

    /// Admin-only: revoke a credential (e.g. score was fraudulently inflated).
    pub fn revoke(env: Env, credential_id: String) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        let mut cred: Credential = env
            .storage()
            .persistent()
            .get(&DataKey::Credential(credential_id.clone()))
            .expect("credential not found");

        cred.revoked = true;
        env.storage()
            .persistent()
            .set(&DataKey::Credential(credential_id), &cred);
    }

    /// Public: returns true if the credential exists, is not revoked, and has not expired.
    pub fn verify(env: Env, credential_id: String) -> bool {
        let cred: Option<Credential> = env
            .storage()
            .persistent()
            .get(&DataKey::Credential(credential_id));

        match cred {
            None => false,
            Some(c) => !c.revoked && env.ledger().sequence() <= c.expires_ledger,
        }
    }

    /// Public: fetch full credential metadata by ID.
    pub fn get_credential(env: Env, credential_id: String) -> Option<Credential> {
        env.storage()
            .persistent()
            .get(&DataKey::Credential(credential_id))
    }

    /// Public: fetch the most recent credential for a wallet.
    pub fn get_latest(env: Env, wallet: Address) -> Option<Credential> {
        let cred_id: Option<String> = env
            .storage()
            .persistent()
            .get(&DataKey::WalletLatest(wallet));
        cred_id.and_then(|id| env.storage().persistent().get(&DataKey::Credential(id)))
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{Bytes, Env, String};

    fn dummy_hash(env: &Env) -> BytesN<32> {
        BytesN::from_array(env, &[0u8; 32])
    }

    #[test]
    fn test_register_and_verify() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, CredentialRegistry);
        let client = CredentialRegistryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let wallet = Address::generate(&env);
        let cred_id = String::from_str(&env, "cred-stellar-742-20260619");

        client.initialize(&admin);
        client.register(&wallet, &cred_id, &742, &1_000_000, &dummy_hash(&env));

        assert!(client.verify(&cred_id));

        let cred = client.get_credential(&cred_id).unwrap();
        assert_eq!(cred.score, 742);
        assert!(!cred.revoked);

        let latest = client.get_latest(&wallet).unwrap();
        assert_eq!(latest.score, 742);
    }

    #[test]
    fn test_revoke() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, CredentialRegistry);
        let client = CredentialRegistryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let wallet = Address::generate(&env);
        let cred_id = String::from_str(&env, "cred-abc");

        client.initialize(&admin);
        client.register(&wallet, &cred_id, &700, &1_000_000, &dummy_hash(&env));
        assert!(client.verify(&cred_id));

        client.revoke(&cred_id);
        assert!(!client.verify(&cred_id));
    }
}
