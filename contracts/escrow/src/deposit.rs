use crate::{emit_status_changed, ttl, Contract, ContractStatus, DataKey, Error, Milestone};
use soroban_sdk::{Address, Env, Symbol, Vec};

/// Deposits funds into the contract. Transitions to Funded status when fully funded.
///
/// # Arguments
/// * `env` - The contract environment
/// * `contract_id` - The contract ID
/// * `caller` - The address of the caller (must be the client)
/// * `amount` - The amount to deposit (in stroops)
///
/// # Returns
/// `true` if deposit was successful
///
/// # Errors
/// * `AmountMustBePositive` - If amount is <= 0
/// * `ContractNotFound` - If contract doesn't exist
/// * `InvalidState` - If contract is not in Created state
/// * `UnauthorizedRole` - If caller is not the client
pub fn deposit_funds_impl(env: &Env, contract_id: u32, caller: Address, amount: i128) -> bool {
    if amount <= 0 {
        env.panic_with_error(Error::AmountMustBePositive);
    }

    let mut contract: Contract = env
        .storage()
        .persistent()
        .get(&DataKey::Contract(contract_id))
        .unwrap_or_else(|| env.panic_with_error(Error::ContractNotFound));

    ttl::extend_contract_ttl(&env, contract_id);

    if caller != contract.client {
        env.panic_with_error(Error::UnauthorizedRole);
    }
    caller.require_auth();

    if contract.status != ContractStatus::Created
        && contract.status != ContractStatus::PartiallyFunded
    {
        env.panic_with_error(Error::InvalidState);
    }

    contract.funded_amount += amount;

    let milestones: Vec<Milestone> = ttl::load_milestones(&env, contract_id);

    let total_amount: i128 = milestones.iter().map(|m| m.amount).sum();

    let old_status = contract.status.clone();
    if contract.funded_amount >= total_amount {
        if old_status == ContractStatus::Created || old_status == ContractStatus::PartiallyFunded {
            contract.status = ContractStatus::Funded;
            emit_status_changed(env, contract_id, old_status, ContractStatus::Funded);
        }
    } else if contract.funded_amount > 0 {
        if old_status == ContractStatus::Created {
            contract.status = ContractStatus::PartiallyFunded;
            emit_status_changed(
                env,
                contract_id,
                old_status,
                ContractStatus::PartiallyFunded,
            );
        }
    }

    env.storage()
        .persistent()
        .set(&DataKey::Contract(contract_id), &contract);

    ttl::extend_contract_ttl(&env, contract_id);

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{create_contract, register_client, total_milestone_amount};
    use soroban_sdk::testutils::Events as _;
    use soroban_sdk::TryFromVal;
    extern crate std;
    use std::format;

    #[test]
    fn deposit_emits_status_changed_event() {
        let env = Env::default();
        env.mock_all_auths();

        let client = register_client(&env);
        let (client_addr, _, contract_id) = create_contract(&env, &client);

        assert!(client.deposit_funds(&contract_id, &client_addr, &total_milestone_amount(),));

        let events = env.events().all();

        assert!(events.iter().any(|e| {
            let topics = &e.1;
            if topics.len() > 0 {
                if let Ok(sym) = Symbol::try_from_val(&env, &topics.get(0).unwrap()) {
                    sym == Symbol::new(&env, "status_changed")
                } else {
                    false
                }
            } else {
                false
            }
        }));
    }
}
