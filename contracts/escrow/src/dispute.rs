// Merged imports

use crate::{
    safe_add_amounts, ttl, Contract, ContractStatus, DataKey, Error, Escrow, EscrowArgs,
    EscrowClient, EscrowError,
};
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

// Removed obsolete duplicated `impl Escrow`

/// Resolution selected by the assigned arbiter for a disputed escrow.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeResolution {
    /// Refund all remaining escrowed funds to the client.
    FullRefund,
    /// Refund 70% of the remaining balance to the client and release 30% to the freelancer.
    PartialRefund,
    /// Release all remaining escrowed funds to the freelancer.
    FullPayout,
    /// Apply a custom split of the remaining balance.
    Split(i128, i128),
}

impl DisputeResolution {
    pub fn code(&self) -> u32 {
        match self {
            Self::FullRefund => 0,
            Self::PartialRefund => 1,
            Self::FullPayout => 2,
            Self::Split(_, _) => 3,
        }
    }
}

#[allow(dead_code)]
pub fn resolution_payouts(
    contract: &Contract,
    resolution: &DisputeResolution,
) -> Result<(i128, i128), EscrowError> {
    let available = contract
        .funded_amount
        .checked_sub(contract.released_amount)
        .and_then(|value| value.checked_sub(contract.refunded_amount))
        .ok_or(EscrowError::AccountingInvariantViolated)?;
    if available < 0 {
        return Err(EscrowError::AccountingInvariantViolated);
    }

    match resolution {
        DisputeResolution::FullRefund => Ok((available, 0)),
        DisputeResolution::PartialRefund => {
            let freelancer_payout = available
                .checked_mul(30)
                .and_then(|value| value.checked_div(100))
                .ok_or(EscrowError::PotentialOverflow)?;
            Ok((available - freelancer_payout, freelancer_payout))
        }
        DisputeResolution::FullPayout => Ok((0, available)),
        DisputeResolution::Split(client_amount, freelancer_amount) => {
            if *client_amount < 0 || *freelancer_amount < 0 {
                return Err(EscrowError::InvalidDisputeSplit);
            }
            let total = safe_add_amounts(*client_amount, *freelancer_amount)
                .ok_or(EscrowError::PotentialOverflow)?;
            if total != available {
                return Err(EscrowError::InvalidDisputeSplit);
            }
            Ok((*client_amount, *freelancer_amount))
        }
    }
}

#[allow(dead_code)]
pub fn final_status_after_resolution(contract: &Contract) -> ContractStatus {
    if contract.refunded_amount == contract.funded_amount {
        ContractStatus::Refunded
    } else {
        ContractStatus::Completed
    }
}

impl Escrow {
    /// Raise a dispute on a funded or partially funded escrow.
    /// Only the client or freelancer may call this.
    pub(crate) fn raise_dispute_impl(env: &Env, contract_id: u32, caller: Address) -> bool {
        Self::require_not_paused(env);
        caller.require_auth();

        let mut contract: Contract = env
            .storage()
            .persistent()
            .get(&DataKey::Contract(contract_id))
            .unwrap_or_else(|| env.panic_with_error(EscrowError::ContractNotFound));

        ttl::extend_contract_ttl(env, contract_id);
        Self::require_not_finalized(env, contract_id);

        // Verify caller is client or freelancer
        if caller != contract.client && caller != contract.freelancer {
            env.panic_with_error(EscrowError::UnauthorizedRole);
        }

        // Require arbiter assignment
        if contract.arbiter.is_none() {
            env.panic_with_error(EscrowError::ArbiterRequired);
        }

        // Verify contract is in a disputable state (Funded or PartiallyFunded)
        match contract.status {
            ContractStatus::Funded | ContractStatus::PartiallyFunded => {}
            _ => env.panic_with_error(EscrowError::InvalidState),
        }

        contract.status = ContractStatus::Disputed;
        env.storage()
            .persistent()
            .set(&DataKey::Contract(contract_id), &contract);

        ttl::extend_contract_ttl(env, contract_id);

        env.events().publish(
            (symbol_short!("dispute"), symbol_short!("opened")),
            (contract_id, caller),
        );
        true
    }

    /// Resolve a disputed escrow. Only the assigned arbiter may call this.
    pub(crate) fn resolve_dispute_impl(
        env: &Env,
        contract_id: u32,
        arbiter: Address,
        resolution: DisputeResolution,
    ) -> bool {
        Self::require_not_paused(env);
        arbiter.require_auth();

        let mut contract: Contract = env
            .storage()
            .persistent()
            .get(&DataKey::Contract(contract_id))
            .unwrap_or_else(|| env.panic_with_error(EscrowError::ContractNotFound));

        ttl::extend_contract_ttl(env, contract_id);
        Self::require_not_finalized(env, contract_id);

        // Verify contract is in Disputed state
        if contract.status != ContractStatus::Disputed {
            env.panic_with_error(EscrowError::InvalidStatusTransition);
        }

        // Verify caller is the assigned arbiter
        match &contract.arbiter {
            Some(contract_arbiter) if *contract_arbiter == arbiter => {}
            _ => env.panic_with_error(EscrowError::UnauthorizedRole),
        }

        // Compute payouts based on resolution
        let (client_payout, freelancer_payout) = resolution_payouts(&contract, &resolution)
            .unwrap_or_else(|err| env.panic_with_error(err));

        contract.refunded_amount = safe_add_amounts(contract.refunded_amount, client_payout)
            .unwrap_or_else(|| env.panic_with_error(EscrowError::PotentialOverflow));
        contract.released_amount = safe_add_amounts(contract.released_amount, freelancer_payout)
            .unwrap_or_else(|| env.panic_with_error(EscrowError::PotentialOverflow));

        if safe_add_amounts(contract.released_amount, contract.refunded_amount)
            != Some(contract.funded_amount)
        {
            env.panic_with_error(EscrowError::AccountingInvariantViolated);
        }

        contract.status = final_status_after_resolution(&contract);
        env.storage()
            .persistent()
            .set(&DataKey::Contract(contract_id), &contract);

        ttl::extend_contract_ttl(env, contract_id);

        env.events().publish(
            (symbol_short!("dsp_res"), contract_id),
            (
                arbiter,
                resolution.code(),
                client_payout,
                freelancer_payout,
                env.ledger().timestamp(),
            ),
        );
        true
    }
}
