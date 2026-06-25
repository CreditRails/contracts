#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

// ── Types ─────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum RiskTier {
    A, // 800–850 — Excellent
    B, // 740–799 — Very Good
    C, // 670–739 — Good
    D, // 580–669 — Fair
    F, // 300–579 — Poor
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

// ── Helpers ───────────────────────────────────────────────────────────────────

fn score_to_tier(score: u32) -> RiskTier {
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
    }

    /// Admin-only: write a new score for a wallet after indexer computation.
    pub fn update_score(env: Env, wallet: Address, score: u32, percentile: u32) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        assert!((300..=850).contains(&score), "score must be 300–850");
        assert!(percentile <= 100, "percentile must be 0–100");

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

        env.storage()
            .persistent()
            .set(&DataKey::Profile(wallet), &profile);
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    #[test]
    fn test_score_tiers() {
        assert_eq!(score_to_tier(850), RiskTier::A);
        assert_eq!(score_to_tier(742), RiskTier::B);
        assert_eq!(score_to_tier(700), RiskTier::C);
        assert_eq!(score_to_tier(620), RiskTier::D);
        assert_eq!(score_to_tier(400), RiskTier::F);
    }

    #[test]
    fn test_update_and_get_score() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, CreditScoreContract);
        let client = CreditScoreContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let wallet = Address::generate(&env);

        client.initialize(&admin);
        client.update_score(&wallet, &742, &81);

        let profile = client.get_score(&wallet).unwrap();
        assert_eq!(profile.score, 742);
        assert_eq!(profile.risk_tier, RiskTier::B);
        assert!(profile.loan_eligible);
        assert_eq!(profile.max_loan_usdc, 12_500);
        assert_eq!(profile.percentile, 81);
    }

    #[test]
    #[should_panic(expected = "score must be 300–850")]
    fn test_invalid_score_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, CreditScoreContract);
        let client = CreditScoreContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let wallet = Address::generate(&env);

        client.initialize(&admin);
        client.update_score(&wallet, &999, &50);
    }
}
