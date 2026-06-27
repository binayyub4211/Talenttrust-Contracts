use crate::{
    DataKey, Escrow, EscrowError, GovernedParameters, ReadinessChecklist,
    ADMIN_ROTATION_MIN_DELAY_LEDGERS,
};
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

use soroban_sdk::{symbol_short, Address, Env, Symbol};

/// Governance-related privileged operations.
impl Escrow {
    // ── Protocol Fee ─────────────────────────────────────────

    pub(crate) fn set_protocol_fee_bps_impl(env: &Env, new_bps: u32) -> bool {
        Self::require_initialized(env);

        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(EscrowError::ContractNotFound));
        admin.require_auth();

        let old_bps: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::ProtocolFeeBps)
            .unwrap_or(0);

        env.storage()
            .persistent()
            .set(&DataKey::ProtocolFeeBps, &new_bps);

        env.events().publish(
            (Symbol::new(env, "protocol_fee_bps"),),
            (old_bps, new_bps, admin.clone(), env.ledger().timestamp()),
        );

        true
    }

    pub(crate) fn get_protocol_fee_bps_impl(env: &Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::ProtocolFeeBps)
            .unwrap_or(0)
    }

    // ── Admin Rotation ───────────────────────────────────────

    pub(crate) fn propose_governance_admin_impl(
        env: &Env,
        proposed: Address,
    ) -> bool {
        Self::require_initialized(env);

        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(EscrowError::ContractNotFound));
        admin.require_auth();

        env.storage().persistent().set(
            &DataKey::PendingAdmin,
            &PendingAdminProposal {
                proposed: proposed.clone(),
                proposed_at_ledger: env.ledger().sequence(),
            },
        );

        env.events().publish(
            (symbol_short!("admin"), Symbol::new(env, "proposed")),
            (admin, proposed, env.ledger().timestamp()),
        );

        true
    }

    pub(crate) fn accept_governance_admin_impl(env: &Env) -> bool {
        Self::require_initialized(env);

        let pending: PendingAdminProposal = env
            .storage()
            .persistent()
            .get(&DataKey::PendingAdmin)
            .unwrap_or_else(|| env.panic_with_error(EscrowError::InvalidState));

        let elapsed = env
            .ledger()
            .sequence()
            .saturating_sub(pending.proposed_at_ledger);

        if elapsed < ADMIN_ROTATION_MIN_DELAY_LEDGERS {
            env.panic_with_error(EscrowError::TimelockNotElapsed);
        }

        let pending_admin = pending.proposed;
        pending_admin.require_auth();

        let old_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| env.panic_with_error(EscrowError::ContractNotFound));

        env.storage()
            .persistent()
            .set(&DataKey::Admin, &pending_admin);

        env.storage()
            .persistent()
            .remove(&DataKey::PendingAdmin);

        env.events().publish(
            (symbol_short!("admin"), Symbol::new(env, "accepted")),
            (old_admin, pending_admin.clone(), env.ledger().timestamp()),
        );

        true
    }

    pub(crate) fn get_pending_governance_admin_impl(
        env: &Env,
    ) -> Option<Address> {
        env.storage()
            .persistent()
            .get::<_, PendingAdminProposal>(&DataKey::PendingAdmin)
            .map(|p| p.proposed)
    }

    pub(crate) fn get_governance_admin_impl(
        env: &Env,
    ) -> Option<Address> {
        env.storage().persistent().get(&DataKey::Admin)
    }
}
