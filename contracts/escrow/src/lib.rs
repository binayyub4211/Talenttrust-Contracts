#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env, Symbol, Vec};

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
}

#[cfg(test)]
mod test;
