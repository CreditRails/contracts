#![cfg(test)]

use crate::{CredentialRegistry, CredentialRegistryClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, BytesN, Env, String};

fn dummy_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}

#[test]
fn test_register_and_verify() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CredentialRegistry, ());
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

    let contract_id = env.register(CredentialRegistry, ());
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

#[test]
fn test_get_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CredentialRegistry, ());
    let client = CredentialRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    assert_eq!(client.get_admin(), admin);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CredentialRegistry, ());
    let client = CredentialRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);
    client.initialize(&admin);
}

#[test]
#[should_panic(expected = "credential not found")]
fn test_revoke_nonexistent_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(CredentialRegistry, ());
    let client = CredentialRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);
    client.revoke(&String::from_str(&env, "does-not-exist"));
}
