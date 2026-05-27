use crate::{types::ProtocolParameters, DataKey, EscrowError};
use soroban_sdk::{contractimpl, symbol_short, Address, Env, Symbol};

impl Escrow {
    fn validate_protocol_parameters(
        env: &Env,
        min_milestone_amount: i128,
        max_milestones: u32,
        min_reputation_rating: i128,
        max_reputation_rating: i128,
    ) -> ProtocolParameters {
        if min_milestone_amount <= 0 {
            env.panic_with_error(EscrowError::InvalidProtocolParameters);
        }
        if max_milestones == 0 {
            env.panic_with_error(EscrowError::InvalidProtocolParameters);
        }
        if min_reputation_rating <= 0 {
            env.panic_with_error(EscrowError::InvalidProtocolParameters);
        }
        if min_reputation_rating > max_reputation_rating {
            env.panic_with_error(EscrowError::InvalidProtocolParameters);
        }

        ProtocolParameters {
            min_milestone_amount,
            max_milestones,
            min_reputation_rating,
            max_reputation_rating,
        }
    }

    fn governance_admin(env: &Env) -> Address {
        env.storage()
            .persistent()
            .get(&DataKey::GovernanceAdmin)
            .unwrap_or_else(|| env.panic_with_error(EscrowError::GovernanceNotInitialized))
    }

    fn set_governance_readiness(env: &Env) {
        let mut checklist = Self::load_checklist(env);
        checklist.governed_params_set = true;
        env.storage()
            .persistent()
            .set(&DataKey::ReadinessChecklist, &checklist);
    }
}

#[contractimpl]
impl Escrow {
    /// One-time governance initialization.
    /// Sets the governance admin and initial protocol parameters.
    pub fn initialize_protocol_governance(
        env: Env,
        admin: Address,
        min_milestone_amount: i128,
        max_milestones: u32,
        min_reputation_rating: i128,
        max_reputation_rating: i128,
    ) -> bool {
        if env
            .storage()
            .persistent()
            .get::<_, Address>(&DataKey::GovernanceAdmin)
            .is_some()
        {
            panic!("protocol governance is already initialized");
        }

        admin.require_auth();
        let params = Self::validate_protocol_parameters(
            &env,
            min_milestone_amount,
            max_milestones,
            min_reputation_rating,
            max_reputation_rating,
        );
        env.storage().persistent().set(&DataKey::GovernanceAdmin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::ProtocolParameters, &params);
        Self::set_governance_readiness(&env);

        env.events().publish(
            (symbol_short!("governance"), Symbol::new(&env, "initialized")),
            (admin.clone(), env.ledger().timestamp()),
        );
        true
    }

    /// Convenience governance initialization using default protocol parameters.
    pub fn initialize_governance(env: Env, admin: Address) -> bool {
        let defaults = ProtocolParameters::default();
        Self::initialize_protocol_governance(
            env,
            admin,
            defaults.min_milestone_amount,
            defaults.max_milestones,
            defaults.min_reputation_rating,
            defaults.max_reputation_rating,
        )
    }

    /// Update governance protocol parameters. Requires governance admin auth.
    pub fn update_protocol_parameters(
        env: Env,
        min_milestone_amount: i128,
        max_milestones: u32,
        min_reputation_rating: i128,
        max_reputation_rating: i128,
    ) -> bool {
        if env
            .storage()
            .persistent()
            .get::<_, Address>(&DataKey::GovernanceAdmin)
            .is_none()
        {
            panic!("protocol governance is not initialized");
        }

        let admin = Self::governance_admin(&env);
        admin.require_auth();

        let params = Self::validate_protocol_parameters(
            &env,
            min_milestone_amount,
            max_milestones,
            min_reputation_rating,
            max_reputation_rating,
        );
        env.storage()
            .persistent()
            .set(&DataKey::ProtocolParameters, &params);
        Self::set_governance_readiness(&env);

        env.events().publish(
            (symbol_short!("governance"), Symbol::new(&env, "parameters_updated")),
            (admin.clone(), env.ledger().timestamp()),
        );
        true
    }

    /// Returns the currently active protocol parameters.
    pub fn get_protocol_parameters(env: Env) -> ProtocolParameters {
        env.storage()
            .persistent()
            .get(&DataKey::ProtocolParameters)
            .unwrap_or_default()
    }

    /// Returns the current governance admin if initialized.
    pub fn get_governance_admin(env: Env) -> Option<Address> {
        env.storage().persistent().get(&DataKey::GovernanceAdmin)
    }

    /// Returns the pending governance admin, if one has been proposed.
    pub fn get_pending_governance_admin(env: Env) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::PendingGovernanceAdmin)
    }

    /// Propose a new governance admin. Current admin must authenticate.
    pub fn propose_governance_admin(env: Env, new_admin: Address) -> bool {
        let admin = Self::governance_admin(&env);
        admin.require_auth();

        env.storage()
            .persistent()
            .set(&DataKey::PendingGovernanceAdmin, &new_admin);
        env.events().publish(
            (
                symbol_short!("governance"),
                Symbol::new(&env, "admin_proposed"),
            ),
            (admin.clone(), new_admin.clone(), env.ledger().timestamp()),
        );
        true
    }

    /// Accept proposed governance admin privileges. Pending admin must authorize.
    pub fn accept_governance_admin(env: Env) -> bool {
        let pending_admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::PendingGovernanceAdmin)
            .unwrap_or_else(|| env.panic_with_error(EscrowError::InvalidState));
        pending_admin.require_auth();

        env.storage()
            .persistent()
            .set(&DataKey::GovernanceAdmin, &pending_admin);
        env.storage()
            .persistent()
            .remove(&DataKey::PendingGovernanceAdmin);
        env.events().publish(
            (
                symbol_short!("governance"),
                Symbol::new(&env, "admin_accepted"),
            ),
            (pending_admin.clone(), env.ledger().timestamp()),
        );
        true
    }
}
