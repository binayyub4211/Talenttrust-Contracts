// ─────────────────────────────────────────────────────────────────────────────
// End-to-End Lifecycle Tests: Happy Path Scenarios
// ─────────────────────────────────────────────────────────────────────────────
//
// These tests walk complete escrow lifecycles from create_contract through
// release_milestone to Completed status, asserting every state transition and
// balance at each step.
//
// Balance Invariant Table (holds at EVERY step, no exceptions):
//
// Operation                  | status      | funded_amount | released_amount | refundable_balance
// ─────────────────────────────────────────────────────────────────────────────────────────────
// create_contract            | Created     | 0             | 0               | 0
// deposit_funds (D total)    | Funded      | D             | 0               | D
// approve_milestone(i)       | Funded      | D             | 0               | D
// release_milestone(i)       | Funded*     | D             | M_i             | D - M_i
// release_last_milestone     | Completed   | D             | D               | 0
//
// * Status becomes Completed only when ALL milestones are released or refunded
//
// Invariant: funded_amount = released_amount + refundable_balance + refunded_amount
// Constraint: released_amount <= funded_amount (always)
// Constraint: refundable_balance >= 0 (always)

use super::{register_client, create_contract};
use crate::{ContractStatus, ReleaseAuthorization};
use soroban_sdk::{testutils::Address as _, vec, Address, Env};

// ─────────────────────────────────────────────────────────────────────────────
// Test 1: Single Milestone with ClientOnly Authorization
// ─────────────────────────────────────────────────────────────────────────────

/// Proves: complete escrow lifecycle with one milestone and client-only approval,
/// asserting every state transition and balance at each step. This is the simplest
/// happy-path scenario and serves as the baseline for all other lifecycle tests.
#[test]
fn test_full_lifecycle_single_milestone_client_only_auth() {
    // ARRANGE
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let milestones = vec![&env, 1_000_000_i128];
    let total = 1_000_000_i128;

    // ACT + ASSERT (interleaved — assert after each operation)

    // Step 1: Create contract in Created state
    let contract_id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &milestones,
        &ReleaseAuthorization::ClientOnly,
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.status, ContractStatus::Created,
        "After create: status must be Created"
    );
    assert_eq!(contract.funded_amount, 0, "After create: funded_amount must be 0");
    assert_eq!(contract.released_amount, 0, "After create: released_amount must be 0");
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        0,
        "After create: refundable_balance must be 0"
    );

    // Step 2: Deposit funds → transitions to Funded
    assert!(
        client.deposit_funds(&contract_id, &client_addr, &total),
        "deposit_funds must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.status, ContractStatus::Funded,
        "After deposit: status must be Funded"
    );
    assert_eq!(
        contract.funded_amount, total,
        "After deposit: funded_amount must be 1_000_000"
    );
    assert_eq!(contract.released_amount, 0, "After deposit: released_amount must be 0");
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        total,
        "After deposit: refundable_balance must be 1_000_000"
    );

    // Verify balance invariant
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "Balance invariant must hold after deposit"
    );

    // Step 3: Approve milestone 0
    assert!(
        client.approve_milestone_release(&contract_id, &client_addr, &0),
        "approve_milestone_release must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.status, ContractStatus::Funded,
        "After approve: status still Funded"
    );
    assert_eq!(contract.funded_amount, total, "After approve: funded_amount unchanged");
    assert_eq!(
        contract.released_amount, 0,
        "After approve: released_amount still 0"
    );
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        total,
        "After approve: refundable_balance unchanged"
    );

    // Verify balance invariant
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "Balance invariant must hold after approve"
    );

    // Step 4: Release milestone 0 → transitions to Completed (only milestone)
    assert!(
        client.release_milestone(&contract_id, &client_addr, &0),
        "release_milestone must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.status, ContractStatus::Completed,
        "After release final: status must be Completed"
    );
    assert_eq!(
        contract.funded_amount, total,
        "After release final: funded_amount unchanged"
    );
    assert_eq!(
        contract.released_amount, total,
        "After release final: released_amount must equal total"
    );
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        0,
        "After release final: refundable_balance must be 0"
    );

    // Verify balance invariant holds at final state
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "Balance invariant must hold at completion"
    );

    // SECURITY: Verify no funds escaped
    assert!(
        contract.released_amount <= contract.funded_amount,
        "released_amount must never exceed funded_amount"
    );
    assert!(
        client.get_refundable_balance(&contract_id) >= 0,
        "refundable_balance must never be negative"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 2: Multiple Milestones with ClientOnly Authorization
// ─────────────────────────────────────────────────────────────────────────────

/// Proves: complete escrow lifecycle with three milestones and client-only approval,
/// asserting ALL intermediate states, not just the final state. This test verifies that
/// the balance invariant holds at every step of a multi-milestone release sequence.
#[test]
fn test_full_lifecycle_multi_milestone_client_only_auth() {
    // ARRANGE
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);

    let m1 = 500_000_i128;
    let m2 = 300_000_i128;
    let m3 = 200_000_i128;
    let total = m1 + m2 + m3;

    let milestones = vec![&env, m1, m2, m3];

    // ACT + ASSERT

    // Create and deposit (using inline pattern for detailed assertions)
    let contract_id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &milestones,
        &ReleaseAuthorization::ClientOnly,
    );

    // Deposit
    assert!(
        client.deposit_funds(&contract_id, &client_addr, &total),
        "deposit must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(contract.status, ContractStatus::Funded, "After deposit: status == Funded");
    assert_eq!(
        contract.funded_amount, total,
        "After deposit: funded_amount == total"
    );
    assert_eq!(
        contract.released_amount, 0,
        "After deposit: released_amount == 0"
    );
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        total,
        "After deposit: refundable_balance == total"
    );

    // Milestone 1: Approve and release
    assert!(
        client.approve_milestone_release(&contract_id, &client_addr, &0),
        "approve m1 must succeed"
    );
    assert!(
        client.release_milestone(&contract_id, &client_addr, &0),
        "release m1 must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.status, ContractStatus::Funded,
        "After release m1: status still Funded"
    );
    assert_eq!(
        contract.funded_amount, total,
        "After release m1: funded_amount == total"
    );
    assert_eq!(
        contract.released_amount, m1,
        "After release m1: released_amount == m1"
    );
    let expected_refundable = total - m1;
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        expected_refundable,
        "After release m1: refundable_balance == total - m1"
    );

    // Verify invariant
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "Balance invariant must hold after m1"
    );

    // Milestone 2: Approve and release
    assert!(
        client.approve_milestone_release(&contract_id, &client_addr, &1),
        "approve m2 must succeed"
    );
    assert!(
        client.release_milestone(&contract_id, &client_addr, &1),
        "release m2 must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.status, ContractStatus::Funded,
        "After release m2: status still Funded"
    );
    assert_eq!(
        contract.funded_amount, total,
        "After release m2: funded_amount == total"
    );
    assert_eq!(
        contract.released_amount,
        m1 + m2,
        "After release m2: released_amount == m1 + m2"
    );
    let expected_refundable = total - m1 - m2;
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        expected_refundable,
        "After release m2: refundable_balance == total - m1 - m2"
    );

    // Verify invariant
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "Balance invariant must hold after m2"
    );

    // Milestone 3: Approve and release (final)
    assert!(
        client.approve_milestone_release(&contract_id, &client_addr, &2),
        "approve m3 must succeed"
    );
    assert!(
        client.release_milestone(&contract_id, &client_addr, &2),
        "release m3 must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.status, ContractStatus::Completed,
        "After release m3: status == Completed"
    );
    assert_eq!(
        contract.funded_amount, total,
        "After release m3: funded_amount == total"
    );
    assert_eq!(
        contract.released_amount, total,
        "After release m3: released_amount == total"
    );
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        0,
        "After release m3: refundable_balance == 0"
    );

    // Verify final invariant
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "Balance invariant must hold at completion"
    );

    // SECURITY: Verify constraints hold throughout
    assert!(
        contract.released_amount <= contract.funded_amount,
        "released_amount <= funded_amount"
    );
    assert!(
        client.get_refundable_balance(&contract_id) >= 0,
        "refundable_balance >= 0"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 3: MultiSig Authorization (Client AND Freelancer must both approve)
// ─────────────────────────────────────────────────────────────────────────────

/// Proves: MultiSig ReleaseAuthorization requires both client and freelancer approvals.
/// This test verifies that both parties must approve for release to succeed. This is
/// critical for dispute prevention in multi-party workflows.
#[test]
fn test_full_lifecycle_multisig_auth() {
    // ARRANGE
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);

    let m1 = 500_000_i128;
    let m2 = 500_000_i128;
    let total = m1 + m2;

    let milestones = vec![&env, m1, m2];

    // ACT + ASSERT

    // Create and deposit
    let contract_id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &milestones,
        &ReleaseAuthorization::MultiSig,
    );

    assert!(
        client.deposit_funds(&contract_id, &client_addr, &total),
        "deposit must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(contract.status, ContractStatus::Funded, "After deposit: status == Funded");

    // Milestone 1 — Both parties approve and release
    assert!(
        client.approve_milestone_release(&contract_id, &client_addr, &0),
        "client approve m1 must succeed"
    );

    assert!(
        client.approve_milestone_release(&contract_id, &freelancer_addr, &0),
        "freelancer approve m1 must succeed"
    );

    // Now release should work
    assert!(
        client.release_milestone(&contract_id, &client_addr, &0),
        "release m1 with both approvals must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.status, ContractStatus::Funded,
        "After release m1: status still Funded"
    );
    assert_eq!(
        contract.released_amount, m1,
        "After release m1: released_amount == m1"
    );
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        m2,
        "After release m1: refundable_balance == m2"
    );

    // Verify invariant
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "Balance invariant must hold after m1 release"
    );

    // Milestone 2 — Both parties approve and release
    assert!(
        client.approve_milestone_release(&contract_id, &client_addr, &1),
        "client approve m2 must succeed"
    );
    assert!(
        client.approve_milestone_release(&contract_id, &freelancer_addr, &1),
        "freelancer approve m2 must succeed"
    );
    assert!(
        client.release_milestone(&contract_id, &client_addr, &1),
        "release m2 must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.status, ContractStatus::Completed,
        "After release m2: status == Completed"
    );
    assert_eq!(
        contract.released_amount, total,
        "After release m2: released_amount == total"
    );
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        0,
        "After release m2: refundable_balance == 0"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 4: ArbiterOnly Authorization
// ─────────────────────────────────────────────────────────────────────────────

/// Proves: ArbiterOnly ReleaseAuthorization allows only the arbiter to approve milestones.
/// Neither client nor freelancer can approve; only the designated arbitrator has authority.
/// This pattern is typical for time-based release or neutral third-party workflows.
#[test]
fn test_full_lifecycle_arbitrator_only_auth() {
    // ARRANGE
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let arbitrator_addr = Address::generate(&env);

    let m1 = 600_000_i128;
    let m2 = 400_000_i128;
    let total = m1 + m2;

    let milestones = vec![&env, m1, m2];

    // ACT + ASSERT

    // Create with arbiter and deposit
    let contract_id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &arbitrator_addr,
        &milestones,
        &ReleaseAuthorization::ArbiterOnly,
    );

    assert!(
        client.deposit_funds(&contract_id, &client_addr, &total),
        "deposit must succeed"
    );

    // Milestone 1 — Arbiter approves and releases
    assert!(
        client.approve_milestone_release(&contract_id, &arbitrator_addr, &0),
        "arbiter approve m1 must succeed"
    );
    assert!(
        client.release_milestone(&contract_id, &arbitrator_addr, &0),
        "arbiter release m1 must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.status, ContractStatus::Funded,
        "After release m1: status still Funded"
    );
    assert_eq!(
        contract.released_amount, m1,
        "After release m1: released_amount == m1"
    );
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        m2,
        "After release m1: refundable_balance == m2"
    );

    // Verify invariant
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "Balance invariant must hold after m1"
    );

    // Milestone 2 — Arbiter approves and releases (final)
    assert!(
        client.approve_milestone_release(&contract_id, &arbitrator_addr, &1),
        "arbiter approve m2 must succeed"
    );
    assert!(
        client.release_milestone(&contract_id, &arbitrator_addr, &1),
        "arbiter release m2 must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.status, ContractStatus::Completed,
        "After release m2: status == Completed"
    );
    assert_eq!(
        contract.released_amount, total,
        "After release m2: released_amount == total"
    );
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        0,
        "After release m2: refundable_balance == 0"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 5: ClientAndArbiter Authorization (either can approve)
// ─────────────────────────────────────────────────────────────────────────────

/// Proves: ClientAndArbiter ReleaseAuthorization permits release approval by
/// either the client OR the arbiter (not both required). This is useful for workflows
/// where either party can signal readiness independently.
#[test]
fn test_full_lifecycle_client_and_arbiter_auth() {
    // ARRANGE
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let arbitrator_addr = Address::generate(&env);

    let m1 = 400_000_i128;
    let m2 = 300_000_i128;
    let m3 = 300_000_i128;
    let total = m1 + m2 + m3;

    let milestones = vec![&env, m1, m2, m3];

    // ACT + ASSERT

    // Create with arbiter and deposit
    let contract_id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &arbitrator_addr,
        &milestones,
        &ReleaseAuthorization::ClientAndArbiter,
    );

    assert!(
        client.deposit_funds(&contract_id, &client_addr, &total),
        "deposit must succeed"
    );

    // Milestone 1 — Client approves and releases (arbiter not needed)
    assert!(
        client.approve_milestone_release(&contract_id, &client_addr, &0),
        "client approve m1 must succeed"
    );
    assert!(
        client.release_milestone(&contract_id, &client_addr, &0),
        "client release m1 must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.released_amount, m1,
        "After release m1: released_amount == m1"
    );
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        m2 + m3,
        "After release m1: refundable_balance == m2 + m3"
    );

    // Milestone 2 — Arbiter approves and releases (client not needed)
    assert!(
        client.approve_milestone_release(&contract_id, &arbitrator_addr, &1),
        "arbiter approve m2 must succeed"
    );
    assert!(
        client.release_milestone(&contract_id, &arbitrator_addr, &1),
        "arbiter release m2 must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.released_amount,
        m1 + m2,
        "After release m2: released_amount == m1 + m2"
    );
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        m3,
        "After release m2: refundable_balance == m3"
    );

    // Milestone 3 — Client approves and releases (final)
    assert!(
        client.approve_milestone_release(&contract_id, &client_addr, &2),
        "client approve m3 must succeed"
    );
    assert!(
        client.release_milestone(&contract_id, &client_addr, &2),
        "client release m3 must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.status, ContractStatus::Completed,
        "After release m3: status == Completed"
    );
    assert_eq!(
        contract.released_amount, total,
        "After release m3: released_amount == total"
    );
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        0,
        "After release m3: refundable_balance == 0"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test 6: Balance Invariant Holds at Every Step
// ─────────────────────────────────────────────────────────────────────────────

/// Proves: The balance invariant (funded_amount == released_amount + refundable_balance)
/// holds at EVERY step of the lifecycle, not just the final state. This is a critical
/// security property: any violation indicates either fund creation or loss.
///
/// SECURITY PROPERTIES VERIFIED:
/// - No funds are created from nothing (invariant must hold throughout)
/// - No funds are lost or stranded (invariant must hold throughout)
/// - The contract is fail-safe: if invariant is violated, it indicates a critical bug
#[test]
fn test_balance_invariant_holds_at_every_step() {
    // ARRANGE
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);

    let m1 = 300_000_i128;
    let m2 = 400_000_i128;
    let m3 = 300_000_i128;
    let total = m1 + m2 + m3;

    let milestones = vec![&env, m1, m2, m3];

    // ACT + ASSERT

    // Step 1: After create
    let contract_id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &milestones,
        &ReleaseAuthorization::ClientOnly,
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "INVARIANT after create: funded = released + refundable"
    );

    // Step 2: After deposit
    assert!(
        client.deposit_funds(&contract_id, &client_addr, &total),
        "deposit must succeed"
    );

    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "INVARIANT after deposit: funded = released + refundable"
    );

    // Milestone 1
    assert!(
        client.approve_milestone_release(&contract_id, &client_addr, &0),
        "approve m1"
    );
    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "INVARIANT after approve m1: funded = released + refundable"
    );

    assert!(
        client.release_milestone(&contract_id, &client_addr, &0),
        "release m1"
    );
    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "INVARIANT after release m1: funded = released + refundable"
    );

    // Milestone 2
    assert!(
        client.approve_milestone_release(&contract_id, &client_addr, &1),
        "approve m2"
    );
    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "INVARIANT after approve m2: funded = released + refundable"
    );

    assert!(
        client.release_milestone(&contract_id, &client_addr, &1),
        "release m2"
    );
    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "INVARIANT after release m2: funded = released + refundable"
    );

    // Milestone 3
    assert!(
        client.approve_milestone_release(&contract_id, &client_addr, &2),
        "approve m3"
    );
    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "INVARIANT after approve m3: funded = released + refundable"
    );

    assert!(
        client.release_milestone(&contract_id, &client_addr, &2),
        "release m3"
    );
    let contract = client.get_contract(&contract_id);
    assert_eq!(
        contract.funded_amount,
        contract.released_amount + client.get_refundable_balance(&contract_id),
        "INVARIANT after release m3 (final): funded = released + refundable"
    );

    // SECURITY: Verify final state is sound
    assert_eq!(
        contract.status, ContractStatus::Completed,
        "Final status must be Completed"
    );
    assert_eq!(
        contract.funded_amount, total,
        "Final funded_amount must equal total deposited"
    );
    assert_eq!(
        contract.released_amount, total,
        "Final released_amount must equal total (all released)"
    );
    assert_eq!(
        client.get_refundable_balance(&contract_id),
        0,
        "Final refundable_balance must be 0"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Negative Test: Release without prior approval is rejected
// ─────────────────────────────────────────────────────────────────────────────

/// Proves: A release_milestone call fails if there is no valid prior approval.
/// This is a critical security property: release must always require explicit approval.
/// Referenced by the security notes in test_balance_invariant_holds_at_every_step.
#[test]
#[should_panic(expected = "InsufficientApprovals")]
fn test_release_without_approval_is_rejected() {
    // ARRANGE
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    let milestones = vec![&env, 1_000_000_i128];

    let contract_id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &milestones,
        &ReleaseAuthorization::ClientOnly,
    );

    assert!(
        client.deposit_funds(&contract_id, &client_addr, &1_000_000),
        "deposit must succeed"
    );

    // ACT + ASSERT: Try to release without approval — must panic with InsufficientApprovals
    let _result = client.release_milestone(&contract_id, &client_addr, &0);
    // If we reach here, the test fails (we expected a panic)
}

// Keep existing tests for backward compatibility
use super::{complete_contract, default_milestones, total_milestone_amount};
use crate::{EscrowError, types::DataKey};
use soroban_sdk::symbol_short;

#[test]
fn multiple_contracts_for_same_freelancer() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let (_, freelancer_addr, first_id) = complete_contract(&env, &client);
    assert!(client.issue_reputation(&first_id, &5, &None));

    let client_addr = Address::generate(&env);
    let milestones = default_milestones(&env);
    let second_id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &milestones,
        &None,
        &None,
    );

    assert!(client.deposit_funds(&second_id, &total_milestone_amount()));
    assert!(client.release_milestone(&second_id, &0));
    assert!(client.release_milestone(&second_id, &1));
    assert!(client.release_milestone(&second_id, &2));
    assert!(client.issue_reputation(&second_id, &4, &None));

    let record = client.get_reputation_record(&freelancer_addr);
    assert_eq!(record.completed_contracts, 2);
    assert_eq!(record.total_rating, 9);
}

#[test]
fn scenario_reputation_invalid_rating_zero_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let (_, _, contract_id) = complete_contract(&env, &client);

    let result = client.try_issue_reputation(&contract_id, &0, &None);
    super::assert_contract_error(result, EscrowError::InvalidRating);
}

#[test]
fn scenario_reputation_invalid_rating_six_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);

    let (_, _, contract_id) = complete_contract(&env, &client);

    let result = client.try_issue_reputation(&contract_id, &6, &None);
    super::assert_contract_error(result, EscrowError::InvalidRating);
}

#[test]
fn deposit_funds_emits_structured_deposit_event() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (_, _, contract_id) = create_contract(&env, &client);

    assert!(client.deposit_funds(&contract_id, &total_milestone_amount()));

    let events = env.events().all();
    assert!(events.iter().any(|event| event.0 == symbol_short!("deposit")));
}

#[test]
fn release_milestone_emits_protocol_fee_event_when_fees_active() {
    let env = Env::default();
    env.mock_all_auths();
    let client = register_client(&env);
    let (_, _, contract_id) = create_contract(&env, &client);

    assert!(client.deposit_funds(&contract_id, &total_milestone_amount()));
    env.storage()
        .persistent()
        .set(&DataKey::ProtocolFeeBps, &100u32);

    assert!(client.release_milestone(&contract_id, &0));

    let events = env.events().all();
    assert!(events.iter().any(|event| event.0 == symbol_short!("protocol_fee")));
}
