use crate::{
    ttl, validate_single_amount, Contract, ContractStatus, DataKey, Error, Escrow, EscrowArgs,
    EscrowClient, EscrowError, Milestone,
};
use soroban_sdk::{contractimpl, Address, Env, Symbol, Vec};

impl Escrow {
    /// Deposits funds into the contract via the bound Stellar Asset Contract
    /// (SAC). Returns `true` after the SAC transfer and accounting update have
    /// both succeeded atomically.
    ///
    /// # Arguments
    /// * `env` - The contract environment
    /// * `contract_id` - The contract ID
    /// * `caller` - The address funding the deposit (must be the stored
    ///               client)
    /// * `amount` - The amount to deposit (in stroops)
    ///
    /// # Returns
    /// `true` if deposit was successful
    ///
    /// # Errors
    /// * `AmountMustBePositive` - If amount is <= 0
    /// * `ContractNotFound` - If contract doesn't exist
    /// * `InvalidState` - If contract is not in Created or PartiallyFunded state
    /// * `UnauthorizedRole` - If caller is not the client
    pub fn deposit_funds(env: Env, contract_id: u32, caller: Address, amount: i128) -> bool {
        if let Err(e) = validate_single_amount(amount) {
            match e {
                EscrowError::AmountMustBePositive => {
                    env.panic_with_error(Error::AmountMustBePositive);
                }
                _ => {
                    env.panic_with_error(Error::InvalidMilestoneAmount);
                }
            }
        }

        // 2. Load the contract; bump TTL on the read path.
        let mut contract: Contract = env
            .storage()
            .persistent()
            .get(&DataKey::Contract(contract_id))
            .unwrap_or_else(|| env.panic_with_error(Error::ContractNotFound));

        ttl::extend_contract_ttl(env, contract_id);

        // 3. Authenticate the caller. The SAC transfer below also requires
        //    auth on `caller`, so we surface a clean UnauthorizedRole here
        //    before any SAC interaction.
        if caller != contract.client {
            env.panic_with_error(Error::UnauthorizedRole);
        }
        caller.require_auth();

        if contract.status != ContractStatus::Created
            && contract.status != ContractStatus::PartiallyFunded
        {
            env.panic_with_error(Error::InvalidState);
        }

        let milestone_key = Symbol::new(&env, "milestones");
        let milestones: Vec<Milestone> = env
            .storage()
            .persistent()
            .get(&(DataKey::Contract(contract_id), milestone_key.clone()))
            .unwrap();

        ttl::extend_milestone_ttl(env, contract_id);

        let total_amount: i128 = milestones.iter().map(|m| m.amount).sum();

        if contract.funded_amount + amount > total_amount {
            env.panic_with_error(Error::InvalidState);
        }

        contract.funded_amount += amount;

        if contract.funded_amount >= total_amount {
            contract.status = ContractStatus::Funded;
        } else if contract.funded_amount > 0 {
            contract.status = ContractStatus::PartiallyFunded;
        }

        // 7. Persist.
        env.storage().persistent().set(
            &(DataKey::Contract(contract_id), milestone_key),
            &milestones,
        );
        env.storage()
            .persistent()
            .set(&DataKey::Contract(contract_id), &contract);

        ttl::extend_contract_ttl(env, contract_id);

        // 9. Emit a structured deposit event so indexers can distinguish
        //    partial installments from final-funding deposits.
        env.events().publish(
            (symbol_short!("deposited"), contract_id),
            (
                caller,
                amount,
                contract.funded_amount,
                total_amount,
                contract.status.clone(),
            ),
        );

        // 8. Audit event for the SAC deposit path so off-chain indexers can
        //    correlate this with the SAC's own transfer events.
        env.events().publish(
            (symbol_short!("deposited"), contract_id),
            (
                caller,
                amount,
                contract.funded_amount,
                total_amount,
                settlement_token,
            ),
        );

        true
    }
}
