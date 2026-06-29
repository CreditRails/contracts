#![no_std]
use soroban_sdk::{contract, contractevent, contractimpl, contracttype, Address, Env};

const BUMP_THRESHOLD: u32 = 8_640;  // ~12 hours at 5 s/ledger
const BUMP_AMOUNT: u32   = 17_280;  // ~24 hours at 5 s/ledger

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum RiskTier {
    A, // 800-850 — Excellent
    B, // 740-799 — Very Good
    C, // 670-739 — Good
    D, // 580-669 — Fair
    F, // 300-579 — Poor
}

#[contracttype]
#[derive(Clone)]
pub struct CreditProfile {
    pub wallet: Address,
    pub score: u32,
    pub risk_tier: RiskTier,
    pub loan_eligible: bool,
    pub max_loan_usdc: u64,
    pub percentile: u32,
    pub updated_ledger: u32,
}

#[contracttype]
pub enum DataKey {
    Profile(Address),
    Admin,
}

// ── Events ────────────────────────────────────────────────────────────────────

// Emitted each time update_score writes a new profile.
// Topics: ["CreditRls", "score_up"] — both fit within the 9-char symbol limit.
#[contractevent(topics = ["CreditRls", "score_up"], data_format = "single-value")]
struct ScoreUpdated {
    score: u32,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub fn score_to_tier(score: u32) -> RiskTier {
    match score {
        800..=850 => RiskTier::A,
        740..=799 => RiskTier::B,
        670..=739 => RiskTier::C,
        580..=669 => RiskTier::D,
        _ => RiskTier::F,
    }
}

fn tier_to_max_loan(tier: &RiskTier) -> u64 {
    match tier {
        RiskTier::A => 50_000,
        RiskTier::B => 12_500,
        RiskTier::C => 5_000,
        RiskTier::D => 1_000,
        RiskTier::F => 0,
    }
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct CreditScoreContract;

#[contractimpl]
impl CreditScoreContract {
    /// One-time setup — sets the admin (the CreditRails indexer key).
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

    /// Admin-only: write a new score for a wallet after indexer computation.
    pub fn update_score(env: Env, wallet: Address, score: u32, percentile: u32) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        assert!((300..=850).contains(&score), "score must be 300-850");
        assert!(percentile <= 100, "percentile must be 0-100");

        let tier = score_to_tier(score);
        let max_loan = tier_to_max_loan(&tier);

        let profile = CreditProfile {
            wallet: wallet.clone(),
            score,
            risk_tier: tier,
            loan_eligible: score >= 600,
            max_loan_usdc: max_loan,
            percentile,
            updated_ledger: env.ledger().sequence(),
        };

        let key = DataKey::Profile(wallet);
        env.storage().persistent().set(&key, &profile);
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_AMOUNT);
        env.storage()
            .instance()
            .extend_ttl(BUMP_THRESHOLD, BUMP_AMOUNT);

        ScoreUpdated { score }.publish(&env);
    }

    /// Public read — anyone can verify a wallet's on-chain credit profile.
    pub fn get_score(env: Env, wallet: Address) -> Option<CreditProfile> {
        env.storage().persistent().get(&DataKey::Profile(wallet))
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }
}

mod test;
