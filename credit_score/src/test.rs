#![cfg(test)]

use crate::{score_to_tier, CreditScoreContract, CreditScoreContractClient, RiskTier};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

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

    let contract_id = env.register(CreditScoreContract, ());
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
fn test_get_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreditScoreContract, ());
    let client = CreditScoreContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert_eq!(client.get_admin(), admin);
}

#[test]
#[should_panic(expected = "score must be 300-850")]
fn test_invalid_score_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreditScoreContract, ());
    let client = CreditScoreContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let wallet = Address::generate(&env);

    client.initialize(&admin);
    client.update_score(&wallet, &999, &50);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreditScoreContract, ());
    let client = CreditScoreContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);
    client.initialize(&admin); // should panic
}

#[test]
#[should_panic]
fn test_update_score_without_admin_auth_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CreditScoreContract, ());
    let client = CreditScoreContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let wallet = Address::generate(&env);
    client.initialize(&admin);

    // No auth mocked for this call — proves update_score is actually
    // gated by admin.require_auth(), not just producing correct output
    // when auth happens to be mocked.
    env.set_auths(&[]);
    client.update_score(&wallet, &700, &50);
}
