#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, Address, BytesN, Env, String};

const BUMP_THRESHOLD: u32 = 8_640;  // ~12 hours at 5 s/ledger
const BUMP_AMOUNT: u32   = 17_280;  // ~24 hours at 5 s/ledger

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
    Credential(String),    // credential_id -> Credential
    WalletLatest(Address), // wallet -> latest credential_id
    Admin,
}

// ── Events ────────────────────────────────────────────────────────────────────

// Emitted when a new credential is anchored on-chain.
#[contractevent(topics = ["CreditRls", "cred_reg"], data_format = "single-value")]
struct CredentialRegistered {
    score: u32,
}

// Emitted when a credential is revoked by the admin.
#[contractevent(topics = ["CreditRls", "cred_rev"], data_format = "single-value")]
struct CredentialRevoked {
    score: u32,
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
        env.storage()
            .instance()
            .extend_ttl(BUMP_THRESHOLD, BUMP_AMOUNT);
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

        assert!((300..=850).contains(&score), "score must be 300-850");
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

        let cred_key = DataKey::Credential(credential_id.clone());
        env.storage().persistent().set(&cred_key, &cred);
        env.storage()
            .persistent()
            .extend_ttl(&cred_key, BUMP_THRESHOLD, BUMP_AMOUNT);

        let wallet_key = DataKey::WalletLatest(wallet);
        env.storage().persistent().set(&wallet_key, &credential_id);
        env.storage()
            .persistent()
            .extend_ttl(&wallet_key, BUMP_THRESHOLD, BUMP_AMOUNT);

        env.storage()
            .instance()
            .extend_ttl(BUMP_THRESHOLD, BUMP_AMOUNT);

        CredentialRegistered { score }.publish(&env);
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

        let key = DataKey::Credential(credential_id);
        env.storage().persistent().set(&key, &cred);
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);

        CredentialRevoked { score: cred.score }.publish(&env);
    }

    /// Public: returns true if the credential exists, is not revoked, and has not expired.
    pub fn verify(env: Env, credential_id: String) -> bool {
        match env
            .storage()
            .persistent()
            .get::<_, Credential>(&DataKey::Credential(credential_id))
        {
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

mod test;
