#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol, Vec};

pub const MAINNET_PROTOCOL_VERSION: u32 = 1u32;
pub const MAINNET_MAX_TOTAL_ESCROW_PER_CONTRACT_STROOPS: i128 = 1_000_000_000_000_000i128;

mod types;
pub use crate::types::{MainnetReadinessInfo, ReadinessChecklist};
use crate::types::DataKey as ReadinessDataKey;

#[contract]
pub struct Escrow;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EscrowError {
    InvalidParticipant = 1,
    EmptyMilestones = 2,
    InvalidMilestoneAmount = 3,
    InvalidDepositAmount = 4,
    InvalidMilestone = 5,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractData {
    pub client: Address,
    pub freelancer: Address,
    pub milestones: Vec<i128>,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    NextId,
    Contract(u32),
}

fn update_readiness_checklist<F>(env: &Env, f: F)
where
    F: FnOnce(&mut ReadinessChecklist),
{
    let mut checklist: ReadinessChecklist = env
        .storage()
        .instance()
        .get(&ReadinessDataKey::ReadinessChecklist)
        .unwrap_or_default();
    f(&mut checklist);
    env.storage()
        .instance()
        .set(&ReadinessDataKey::ReadinessChecklist, &checklist);
}

#[contractimpl]
impl Escrow {
    /// Hello-world style function for testing and CI.
    pub fn hello(_env: Env, to: Symbol) -> Symbol {
        to
    }

    pub fn create_contract(
        env: Env,
        client: Address,
        freelancer: Address,
        milestones: Vec<i128>,
    ) -> u32 {
        if client == freelancer {
            env.panic_with_error(EscrowError::InvalidParticipant);
        }
        if milestones.is_empty() {
            env.panic_with_error(EscrowError::EmptyMilestones);
        }

        for amount in milestones.iter() {
            if amount <= 0 {
                env.panic_with_error(EscrowError::InvalidMilestoneAmount);
            }
        }

        let id = env
            .storage()
            .persistent()
            .get::<_, u32>(&DataKey::NextId)
            .unwrap_or(0);

        let data = ContractData {
            client,
            freelancer,
            milestones,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Contract(id), &data);
        env.storage().persistent().set(&DataKey::NextId, &(id + 1));

        id
    }

    pub fn deposit_funds(env: Env, _contract_id: u32, amount: i128) -> bool {
        if amount <= 0 {
            env.panic_with_error(EscrowError::InvalidDepositAmount);
        }
        true
    }

    pub fn release_milestone(env: Env, contract_id: u32, milestone_index: u32) -> bool {
        let _ = (env, contract_id, milestone_index);
        true
    }

    pub fn initialize(env: Env, _admin: Address) {
        // Prevent double-initialization
        if env
            .storage()
            .instance()
            .get::<_, bool>(&ReadinessDataKey::Initialized)
            .unwrap_or(false)
        {
            panic!("already initialized");
        }
        env.storage()
            .instance()
            .set(&ReadinessDataKey::Initialized, &true);
        update_readiness_checklist(&env, |c| c.initialized = true);
    }

    pub fn initialize_protocol_governance(
        env: Env,
        _admin: Address,
        _min_amount: i128,
        _max_milestones: u32,
        _min_rating: i128,
        _max_rating: i128,
    ) {
        update_readiness_checklist(&env, |c| c.governed_params_set = true);
    }

    pub fn update_protocol_parameters(
        env: Env,
        _min_amount: i128,
        _max_milestones: u32,
        _min_rating: i128,
        _max_rating: i128,
    ) {
        update_readiness_checklist(&env, |c| c.governed_params_set = true);
    }

    pub fn activate_emergency_pause(env: Env) {
        update_readiness_checklist(&env, |c| c.emergency_controls_enabled = true);
    }

    pub fn resolve_emergency(env: Env) {
        update_readiness_checklist(&env, |c| c.emergency_controls_enabled = true);
    }

    pub fn get_mainnet_readiness_info(env: Env) -> MainnetReadinessInfo {
        let checklist: ReadinessChecklist = env
            .storage()
            .instance()
            .get(&ReadinessDataKey::ReadinessChecklist)
            .unwrap_or_default();
        MainnetReadinessInfo {
            caps_set: MAINNET_MAX_TOTAL_ESCROW_PER_CONTRACT_STROOPS > 0,
            governed_params_set: checklist.governed_params_set,
            emergency_controls_enabled: checklist.emergency_controls_enabled,
            initialized: checklist.initialized,
            protocol_version: MAINNET_PROTOCOL_VERSION,
            max_escrow_total_stroops: MAINNET_MAX_TOTAL_ESCROW_PER_CONTRACT_STROOPS,
        }
    }
}

#[cfg(test)]
mod test;
