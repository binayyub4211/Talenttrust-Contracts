use soroban_sdk::{testutils::Address as _, testutils::Events, vec, Address, Env};

use super::{
    assert_contract_error, complete_contract, create_contract, default_milestones, register_client,
    total_milestone_amount,
};
use crate::{ContractStatus, Error, EscrowError, ReleaseAuthorization};

#[test]
fn refund_succeeds_on_funded_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, _freelancer, contract_id) = create_contract(&env, &client);

    assert!(client.deposit_funds(&contract_id, &client_addr, &total_milestone_amount()));

    let refund_ids = vec![&env, 1_u32];
    let refunded = client.refund_unreleased_milestones(&contract_id, &refund_ids);
    assert_eq!(refunded, 400_0000000_i128);

    let contract = client.get_contract(&contract_id);
    assert_eq!(contract.status, ContractStatus::Funded);
}

#[test]
fn rejects_refund_on_cancelled_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, _freelancer, contract_id) = create_contract(&env, &client);

    assert!(client.cancel_contract(&contract_id, &client_addr));

    let refund_ids = vec![&env, 0_u32];
    assert_contract_error(
        client.try_refund_unreleased_milestones(&contract_id, &refund_ids),
        Error::InvalidState,
    );
}

#[test]
fn rejects_refund_on_completed_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (_client_addr, _freelancer, contract_id) = complete_contract(&env, &client);

    let refund_ids = vec![&env, 0_u32];
    assert_contract_error(
        client.try_refund_unreleased_milestones(&contract_id, &refund_ids),
        Error::InvalidState,
    );
}

#[test]
fn rejects_refund_on_finalized_contract() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, _freelancer, contract_id) = complete_contract(&env, &client);

    assert!(client.finalize_contract(&contract_id, &client_addr));

    let refund_ids = vec![&env, 0_u32];
    assert_contract_error(
        client.try_refund_unreleased_milestones(&contract_id, &refund_ids),
        EscrowError::AlreadyFinalized,
    );
}

// ── Status outcome tests (Issue #570) ────────────────────────────────────────

/// Refunding all milestones on a funded contract transitions status to `Refunded`.
#[test]
fn all_milestones_refunded_transitions_to_refunded() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, _freelancer, contract_id) = create_contract(&env, &client);

    assert!(client.deposit_funds(&contract_id, &client_addr, &total_milestone_amount()));

    let refund_ids = vec![&env, 0_u32, 1_u32, 2_u32];
    let total = client.refund_unreleased_milestones(&contract_id, &refund_ids);
    assert_eq!(total, total_milestone_amount());

    let contract = client.get_contract(&contract_id);
    assert_eq!(contract.status, ContractStatus::Refunded);
}

/// Releasing one milestone then refunding the rest transitions status to `Completed`.
#[test]
fn partial_refund_after_release_transitions_to_completed() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, _freelancer, contract_id) = create_contract(&env, &client);

    assert!(client.deposit_funds(&contract_id, &client_addr, &total_milestone_amount()));

    // Release milestone 0
    assert!(client.approve_milestone_release(&contract_id, &client_addr, &0));
    assert!(client.release_milestone(&contract_id, &client_addr, &0));

    // Refund remaining milestones 1 and 2
    let refund_ids = vec![&env, 1_u32, 2_u32];
    client.refund_unreleased_milestones(&contract_id, &refund_ids);

    let contract = client.get_contract(&contract_id);
    assert_eq!(contract.status, ContractStatus::Completed);
}

/// Verifies that the `refunded` event is emitted after a successful refund.
#[test]
fn refunded_event_is_emitted() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, _freelancer, contract_id) = create_contract(&env, &client);

    assert!(client.deposit_funds(&contract_id, &client_addr, &total_milestone_amount()));

    let refund_ids = vec![&env, 0_u32];
    client.refund_unreleased_milestones(&contract_id, &refund_ids);

    let events = env.events().all();
    assert!(
        !events.is_empty(),
        "at least one event must be emitted after refund"
    );
}

// ── Rejection guard tests (Issue #570) ───────────────────────────────────────

/// Duplicate milestone index in the same refund batch is rejected.
#[test]
fn duplicate_index_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, _freelancer, contract_id) = create_contract(&env, &client);

    assert!(client.deposit_funds(&contract_id, &client_addr, &total_milestone_amount()));

    let refund_ids = vec![&env, 0_u32, 0_u32];
    assert_contract_error(
        client.try_refund_unreleased_milestones(&contract_id, &refund_ids),
        Error::DuplicateMilestoneInRefund,
    );
}

/// Attempting to refund an already-released milestone is rejected.
#[test]
fn already_released_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, _freelancer, contract_id) = create_contract(&env, &client);

    assert!(client.deposit_funds(&contract_id, &client_addr, &total_milestone_amount()));

    // Release milestone 0 first
    assert!(client.approve_milestone_release(&contract_id, &client_addr, &0));
    assert!(client.release_milestone(&contract_id, &client_addr, &0));

    // Now try to refund it
    let refund_ids = vec![&env, 0_u32];
    assert_contract_error(
        client.try_refund_unreleased_milestones(&contract_id, &refund_ids),
        Error::AlreadyReleased,
    );
}

/// Refunding more than the available funded balance is rejected.
#[test]
fn insufficient_funds_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (client_addr, _freelancer, contract_id) = create_contract(&env, &client);

    // Deposit less than milestone 0's amount (200_0000000)
    assert!(client.deposit_funds(&contract_id, &client_addr, &100_0000000_i128));

    let refund_ids = vec![&env, 0_u32];
    assert_contract_error(
        client.try_refund_unreleased_milestones(&contract_id, &refund_ids),
        Error::InsufficientFunds,
    );
}
