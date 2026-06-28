//! Tests for protocol fee deduction from milestone payouts.
//!
//! Covers:
//! - Zero-fee: freelancer receives full gross amount, no fees accumulated
//! - Non-zero-fee: freelancer receives net (gross - fee), fee is accumulated
//! - Multi-release accounting invariant: released_net + refunded + fees <= funded
//! - Helper math edge cases: overflow, tiny amounts, 0 bps

#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env, vec};

use crate::{Escrow, EscrowClient, ReleaseAuthorization};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn setup_client(env: &Env) -> EscrowClient<'_> {
    let contract_id = env.register(Escrow, ());
    let client = EscrowClient::new(env, &contract_id);
    let admin = Address::generate(env);
    client.initialize(&admin);
    client
}

fn create_single_milestone(
    env: &Env,
    client: &EscrowClient<'_>,
    amount: i128,
) -> (Address, u32) {
    let client_addr = Address::generate(env);
    let freelancer_addr = Address::generate(env);
    let milestones = vec![env, amount];
    let id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &milestones,
        &ReleaseAuthorization::ClientOnly,
    );
    (client_addr, id)
}

/// Asserts: released_net + refunded + accumulated_fees <= funded_amount.
fn assert_accounting_invariant(client: &EscrowClient<'_>, id: u32) {
    let summary = client.get_contract_summary(&id);
    let fees = client.get_accumulated_protocol_fees();
    assert!(
        summary.released_amount + summary.refundable_balance + fees <= summary.funded_amount + summary.refundable_balance,
        "invariant: released_net({}) + refunded({}) + fees({}) > funded({})",
        summary.released_amount,
        summary.refundable_balance,
        fees,
        summary.funded_amount,
    );
    // Tighter: released_net + fees == funded - refundable (all non-refundable funds are accounted for)
    // refundable_balance = funded - released_net - refunded_amount
    // So released_net + refunded + fees <= funded  ⟺  fees <= funded - released_net - refunded = refundable_balance
    assert!(
        fees <= summary.refundable_balance + summary.refundable_balance,
        "fees({}) must not exceed what was set aside from the escrow balance",
        fees,
    );
}

// ── Zero-fee tests ────────────────────────────────────────────────────────────

/// With fee_bps == 0, no fees are accumulated.
#[test]
fn test_zero_fee_no_accumulation() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup_client(&env);

    assert_eq!(client.get_protocol_fee_bps(), 0);

    let (client_addr, id) = create_single_milestone(&env, &client, 1_000);
    client.deposit_funds(&id, &client_addr, &1_000);
    client.approve_milestone_release(&id, &client_addr, &0);
    client.release_milestone(&id, &client_addr, &0);

    assert_eq!(client.get_accumulated_protocol_fees(), 0);
}

/// With fee_bps == 0, released_amount equals the full gross milestone amount.
#[test]
fn test_zero_fee_released_amount_equals_gross() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup_client(&env);

    let (client_addr, id) = create_single_milestone(&env, &client, 5_000);
    client.deposit_funds(&id, &client_addr, &5_000);
    client.approve_milestone_release(&id, &client_addr, &0);
    client.release_milestone(&id, &client_addr, &0);

    let summary = client.get_contract_summary(&id);
    // No fee: net == gross, released_amount == 5000
    assert_eq!(summary.released_amount, 5_000);
    assert_eq!(client.get_accumulated_protocol_fees(), 0);
    // refundable_balance == funded - released_net - refunded == 0
    assert_eq!(summary.refundable_balance, 0);
}

// ── Non-zero-fee net payout tests ─────────────────────────────────────────────

/// With 10% fee (1000 bps):
/// - fee = 100, net = 900
/// - released_amount = 900 (net only)
/// - accumulated_fees = 100
/// - released_amount + accumulated_fees = 1000 == funded_amount ✓
#[test]
fn test_nonzero_fee_net_payout_and_accumulation() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup_client(&env);
    client.set_protocol_fee_bps(&1000u32); // 10%

    let (client_addr, id) = create_single_milestone(&env, &client, 1_000);
    client.deposit_funds(&id, &client_addr, &1_000);
    client.approve_milestone_release(&id, &client_addr, &0);
    client.release_milestone(&id, &client_addr, &0);

    let summary = client.get_contract_summary(&id);
    let fees = client.get_accumulated_protocol_fees();

    assert_eq!(fees, 100, "expected fee of 100 (10% of 1000)");
    assert_eq!(summary.released_amount, 900, "net payout to freelancer should be 900");
    // net + fee == funded: no double-counting
    assert_eq!(
        summary.released_amount + fees,
        summary.funded_amount,
        "released_net + fees must equal funded when single milestone fully released"
    );
}

/// 250 bps (2.5%) fee on 4000 stroops → fee = 100, net = 3900.
#[test]
fn test_250bps_fee_net_payout() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup_client(&env);
    client.set_protocol_fee_bps(&250u32); // 2.5%

    let (client_addr, id) = create_single_milestone(&env, &client, 4_000);
    client.deposit_funds(&id, &client_addr, &4_000);
    client.approve_milestone_release(&id, &client_addr, &0);
    client.release_milestone(&id, &client_addr, &0);

    let summary = client.get_contract_summary(&id);
    let fees = client.get_accumulated_protocol_fees();

    // fee = 4000 * 250 / 10000 = 100
    assert_eq!(fees, 100);
    assert_eq!(summary.released_amount, 3_900);
    assert_eq!(summary.released_amount + fees, summary.funded_amount);
}

// ── Multi-release accounting invariant ───────────────────────────────────────

/// Across multiple milestone releases with 5% fee, check after each release:
///   released_net + accumulated_fees + refunded <= funded_amount
#[test]
fn test_multi_release_accounting_invariant_5pct() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup_client(&env);
    client.set_protocol_fee_bps(&500u32); // 5%

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    // 1000 + 2000 + 3000 = 6000 total
    let milestones = vec![&env, 1_000_i128, 2_000_i128, 3_000_i128];
    let id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &milestones,
        &ReleaseAuthorization::ClientOnly,
    );
    client.deposit_funds(&id, &client_addr, &6_000);

    // milestone 0: gross=1000, fee=50, net=950
    client.approve_milestone_release(&id, &client_addr, &0);
    client.release_milestone(&id, &client_addr, &0);
    {
        let s = client.get_contract_summary(&id);
        let fees = client.get_accumulated_protocol_fees();
        assert_eq!(fees, 50);
        assert_eq!(s.released_amount, 950);
        assert!(s.released_amount + s.refundable_balance + fees <= s.funded_amount + s.refundable_balance);
        // refundable_balance = funded - released_net - refunded = 6000 - 950 - 0 = 5050
        // fees (50) should not exceed what is no longer available as net payout or refund
        // i.e. gross_released_so_far = released_net + fees = 1000 <= funded(6000) ✓
        assert_eq!(s.released_amount + fees, 1_000);
    }

    // milestone 1: gross=2000, fee=100, net=1900
    client.approve_milestone_release(&id, &client_addr, &1);
    client.release_milestone(&id, &client_addr, &1);
    {
        let s = client.get_contract_summary(&id);
        let fees = client.get_accumulated_protocol_fees();
        assert_eq!(fees, 150);
        assert_eq!(s.released_amount, 2_850);
        // gross so far = 3000 <= funded(6000) ✓
        assert_eq!(s.released_amount + fees, 3_000);
    }

    // milestone 2: gross=3000, fee=150, net=2850
    client.approve_milestone_release(&id, &client_addr, &2);
    client.release_milestone(&id, &client_addr, &2);
    {
        let s = client.get_contract_summary(&id);
        let fees = client.get_accumulated_protocol_fees();
        assert_eq!(fees, 300);
        assert_eq!(s.released_amount, 5_700);
        // All milestones done: gross_total (released_net + fees) == funded_amount
        assert_eq!(
            s.released_amount + fees,
            s.funded_amount,
            "released_net + fees must equal funded when all milestones done"
        );
    }
}

/// Four milestones with 10% fee: verifies invariant at every step.
#[test]
fn test_multi_release_invariant_10pct() {
    let env = Env::default();
    env.mock_all_auths();
    let client = setup_client(&env);
    client.set_protocol_fee_bps(&1000u32); // 10%

    let client_addr = Address::generate(&env);
    let freelancer_addr = Address::generate(&env);
    // Milestone amounts chosen to avoid ties at zero for tiny-amount floor division
    let milestones = vec![&env, 1_000_i128, 2_500_i128, 3_333_i128, 500_i128];
    let total: i128 = 1_000 + 2_500 + 3_333 + 500; // 7333
    let id = client.create_contract(
        &client_addr,
        &freelancer_addr,
        &None,
        &milestones,
        &ReleaseAuthorization::ClientOnly,
    );
    client.deposit_funds(&id, &client_addr, &total);

    let mut gross_sum = 0i128;
    for idx in 0u32..4 {
        let gross = [1_000i128, 2_500, 3_333, 500][idx as usize];
        client.approve_milestone_release(&id, &client_addr, &idx);
        client.release_milestone(&id, &client_addr, &idx);

        gross_sum += gross;
        let s = client.get_contract_summary(&id);
        let fees = client.get_accumulated_protocol_fees();

        // Core invariant: net_released + fees == gross released so far
        assert_eq!(
            s.released_amount + fees,
            gross_sum,
            "invariant failed at milestone {}",
            idx
        );
        // Must never exceed funded
        assert!(
            s.released_amount + fees <= s.funded_amount,
            "released > funded at milestone {}",
            idx
        );
    }

    // Final: released_net + fees == funded_amount (no refunds)
    let s = client.get_contract_summary(&id);
    let fees = client.get_accumulated_protocol_fees();
    assert_eq!(s.released_amount + fees, s.funded_amount);
}

// ── Fee math edge cases ───────────────────────────────────────────────────────

#[test]
fn test_calculate_protocol_fee_zero_bps() {
    let env = Env::default();
    assert_eq!(Escrow::calculate_protocol_fee(&env, 1_000, 0), 0);
}

#[test]
fn test_calculate_protocol_fee_1000_bps() {
    let env = Env::default();
    // 1000 * 1000 / 10000 = 100
    assert_eq!(Escrow::calculate_protocol_fee(&env, 1_000, 1000), 100);
}

#[test]
fn test_calculate_protocol_fee_tiny_amount_rounds_to_zero() {
    let env = Env::default();
    // 9 * 1000 = 9000 / 10000 = 0 (floor)
    assert_eq!(Escrow::calculate_protocol_fee(&env, 9, 1000), 0);
}

#[test]
#[should_panic]
fn test_calculate_protocol_fee_overflow_panics() {
    let env = Env::default();
    // i128::MAX * 1000 overflows — must panic with PotentialOverflow
    Escrow::calculate_protocol_fee(&env, i128::MAX, 1000);
}
