#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, BytesN, Env,
    Symbol, Vec,
};

mod ttl;

pub use ttl::{
    LEDGERS_PER_DAY, PENDING_APPROVAL_BUMP_THRESHOLD, PENDING_APPROVAL_TTL_LEDGERS,
    PENDING_MIGRATION_BUMP_THRESHOLD, PENDING_MIGRATION_TTL_LEDGERS,
};

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
    PendingApprovalExists = 6,
    PendingApprovalNotFound = 7,
    PendingMigrationExists = 8,
    PendingMigrationNotFound = 9,
    Unauthorized = 10,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractData {
    pub client: Address,
    pub freelancer: Address,
    pub milestones: Vec<i128>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingApproval {
    pub approver: Address,
    pub contract_id: u32,
    pub requested_at_ledger: u32,
    pub expires_at_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingMigration {
    pub proposer: Address,
    pub new_wasm_hash: BytesN<32>,
    pub requested_at_ledger: u32,
    pub expires_at_ledger: u32,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    NextId,
    Contract(u32),
    PendingApproval(u32),
    PendingMigration,
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

    pub fn request_approval(env: Env, approver: Address, contract_id: u32) -> PendingApproval {
        approver.require_auth();

        let key = DataKey::PendingApproval(contract_id);
        if ttl::has_transient(&env, &key) {
            env.panic_with_error(EscrowError::PendingApprovalExists);
        }

        let requested_at_ledger = env.ledger().sequence();
        let expires_at_ledger = ttl::compute_expiry(&env, PENDING_APPROVAL_TTL_LEDGERS);

        let pending = PendingApproval {
            approver: approver.clone(),
            contract_id,
            requested_at_ledger,
            expires_at_ledger,
        };

        ttl::store_with_ttl(&env, &key, &pending, PENDING_APPROVAL_TTL_LEDGERS);

        env.events().publish(
            (symbol_short!("ttl"), symbol_short!("requested")),
            (
                symbol_short!("approval"),
                contract_id,
                approver,
                requested_at_ledger,
                expires_at_ledger,
            ),
        );

        pending
    }

    pub fn cancel_approval(env: Env, approver: Address, contract_id: u32) {
        approver.require_auth();

        let key = DataKey::PendingApproval(contract_id);
        let pending: PendingApproval = ttl::read_if_live(&env, &key)
            .unwrap_or_else(|| env.panic_with_error(EscrowError::PendingApprovalNotFound));

        if pending.approver != approver {
            env.panic_with_error(EscrowError::Unauthorized);
        }

        ttl::remove_transient(&env, &key);

        env.events().publish(
            (symbol_short!("ttl"), symbol_short!("cancelled")),
            (symbol_short!("approval"), contract_id, approver),
        );
    }

    pub fn get_pending_approval(env: Env, contract_id: u32) -> Option<PendingApproval> {
        ttl::read_if_live(&env, &DataKey::PendingApproval(contract_id))
    }

    pub fn extend_pending_approval(env: Env, approver: Address, contract_id: u32) -> bool {
        approver.require_auth();

        let key = DataKey::PendingApproval(contract_id);
        ttl::extend_if_below_threshold(
            &env,
            &key,
            PENDING_APPROVAL_BUMP_THRESHOLD,
            PENDING_APPROVAL_TTL_LEDGERS,
        )
    }

    pub fn request_migration(
        env: Env,
        proposer: Address,
        new_wasm_hash: BytesN<32>,
    ) -> PendingMigration {
        proposer.require_auth();

        let key = DataKey::PendingMigration;
        if ttl::has_transient(&env, &key) {
            env.panic_with_error(EscrowError::PendingMigrationExists);
        }

        let requested_at_ledger = env.ledger().sequence();
        let expires_at_ledger = ttl::compute_expiry(&env, PENDING_MIGRATION_TTL_LEDGERS);

        let pending = PendingMigration {
            proposer: proposer.clone(),
            new_wasm_hash: new_wasm_hash.clone(),
            requested_at_ledger,
            expires_at_ledger,
        };

        ttl::store_with_ttl(&env, &key, &pending, PENDING_MIGRATION_TTL_LEDGERS);

        env.events().publish(
            (symbol_short!("ttl"), symbol_short!("requested")),
            (
                symbol_short!("migration"),
                proposer,
                new_wasm_hash,
                requested_at_ledger,
                expires_at_ledger,
            ),
        );

        pending
    }

    pub fn confirm_migration(env: Env, confirmer: Address) {
        confirmer.require_auth();

        let key = DataKey::PendingMigration;
        let pending: PendingMigration = ttl::read_if_live(&env, &key)
            .unwrap_or_else(|| env.panic_with_error(EscrowError::PendingMigrationNotFound));

        ttl::remove_transient(&env, &key);

        env.events().publish(
            (symbol_short!("ttl"), symbol_short!("confirmed")),
            (
                symbol_short!("migration"),
                confirmer,
                pending.new_wasm_hash,
            ),
        );
    }

    pub fn cancel_migration(env: Env, proposer: Address) {
        proposer.require_auth();

        let key = DataKey::PendingMigration;
        let pending: PendingMigration = ttl::read_if_live(&env, &key)
            .unwrap_or_else(|| env.panic_with_error(EscrowError::PendingMigrationNotFound));

        if pending.proposer != proposer {
            env.panic_with_error(EscrowError::Unauthorized);
        }

        ttl::remove_transient(&env, &key);

        env.events().publish(
            (symbol_short!("ttl"), symbol_short!("cancelled")),
            (symbol_short!("migration"), proposer),
        );
    }

    pub fn get_pending_migration(env: Env) -> Option<PendingMigration> {
        ttl::read_if_live(&env, &DataKey::PendingMigration)
    }

    pub fn extend_pending_migration(env: Env, proposer: Address) -> bool {
        proposer.require_auth();

        let key = DataKey::PendingMigration;
        ttl::extend_if_below_threshold(
            &env,
            &key,
            PENDING_MIGRATION_BUMP_THRESHOLD,
            PENDING_MIGRATION_TTL_LEDGERS,
        )
    }
}

#[cfg(test)]
mod test;
