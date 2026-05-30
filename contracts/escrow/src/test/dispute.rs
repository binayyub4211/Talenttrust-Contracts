//! Escrow dispute helper tests for arbiter assignment and dispute setup.
#![cfg(test)]

use soroban_sdk::{testutils::Address as _, vec, Address, Env};

use crate::{ContractStatus, DataKey, Escrow, EscrowClient, EscrowError};

fn register_client(env: &Env) -> EscrowClient {
    let id = env.register(Escrow, ());
    EscrowClient::new(env, &id)
}

fn create_default_contract(
    env: &Env,
    client: &EscrowClient,
    client_addr: &Address,
    freelancer_addr: &Address,
    arbiter_addr: &Option<Address>,
) -> u32 {
    let milestones = vec![env, 100_i128, 200_i128, 300_i128];
    client.create_contract(client_addr, freelancer_addr, arbiter_addr, &milestones)
}

fn pause_assignments(env: &Env) {
    env.storage().persistent().set(&DataKey::Paused, &true);
}

#[test]
fn client_can_assign_arbiter_while_created() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let arbiter_addr = Address::generate(&env);

    let contract_id = create_default_contract(&env, &client, &client_addr, &freelancer_addr, &None);

    assert!(client.assign_arbiter(&contract_id, &client_addr, &arbiter_addr));

    let contract = client.get_contract(&contract_id);
    assert_eq!(contract.arbiter, Some(arbiter_addr));
    assert_eq!(contract.status, ContractStatus::Created);
}

#[test]
fn freelancer_can_assign_arbiter_after_funding() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let arbiter_addr = Address::generate(&env);

    let contract_id = create_default_contract(&env, &client, &client_addr, &freelancer_addr, &None);
    assert!(client.deposit_funds(&contract_id, &600_i128));

    assert!(client.assign_arbiter(&contract_id, &freelancer_addr, &arbiter_addr));

    let contract = client.get_contract(&contract_id);
    assert_eq!(contract.arbiter, Some(arbiter_addr));
    assert_eq!(contract.status, ContractStatus::Funded);
}

#[test]
fn assign_arbiter_rejects_double_assignment() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let arbiter_addr = Address::generate(&env);
    let second_arbiter = Address::generate(&env);

    let contract_id = create_default_contract(&env, &client, &client_addr, &freelancer_addr, &None);
    assert!(client.assign_arbiter(&contract_id, &client_addr, &arbiter_addr));

    let result = client.try_assign_arbiter(&contract_id, &client_addr, &second_arbiter);
    assert_eq!(result, Err(Ok(EscrowError::ArbiterAlreadyAssigned)));
}

#[test]
fn assign_arbiter_rejects_same_party() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);

    let contract_id = create_default_contract(&env, &client, &client_addr, &freelancer_addr, &None);

    let result = client.try_assign_arbiter(&contract_id, &client_addr, &client_addr);
    assert_eq!(result, Err(Ok(EscrowError::InvalidParticipant)));

    let result = client.try_assign_arbiter(&contract_id, &freelancer_addr, &freelancer_addr);
    assert_eq!(result, Err(Ok(EscrowError::InvalidParticipant)));
}

#[test]
fn assign_arbiter_rejects_unauthorized_caller() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    let arbiter_addr = Address::generate(&env);

    let contract_id = create_default_contract(&env, &client, &client_addr, &freelancer_addr, &None);

    let result = client.try_assign_arbiter(&contract_id, &unauthorized, &arbiter_addr);
    assert_eq!(result, Err(Ok(EscrowError::UnauthorizedRole)));
}

#[test]
fn assign_arbiter_rejects_when_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let arbiter_addr = Address::generate(&env);

    let contract_id = create_default_contract(&env, &client, &client_addr, &freelancer_addr, &None);
    pause_assignments(&env);

    let result = client.try_assign_arbiter(&contract_id, &client_addr, &arbiter_addr);
    assert_eq!(result, Err(Ok(EscrowError::ContractPaused)));
}
