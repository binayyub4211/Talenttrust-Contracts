# Storage Layout Reference — TalentTrust Escrow Contract

This document maps the currently implemented `DataKey` storage used by
`contracts/escrow/src/lib.rs`. A fuller key-by-key reference, including
declared-but-unused keys, is tracked in
[#342](https://github.com/Talenttrust/Talenttrust-Contracts/issues/342).

## Live Storage Keys

These participant indexes are **append-only**: every `create_contract` appends the new id to the appropriate index vectors.
The contract list readers (`list_contracts_by_participant`) are therefore consistent with contract creation order.



| Key | Value | Written by |
| --- | --- | --- |
| `Initialized` | `bool` | `initialize` |
| `Admin` | `Address` | `initialize` |
| `Paused` | `bool` | `pause`, `unpause`, emergency controls |
| `Emergency` | `bool` | emergency controls |
| `Contract(id)` | `EscrowContractData` | create/deposit/release/reputation/cancel |
| `(Contract(id), "milestones")` | `Vec<Milestone>` | create/deposit/release/refund |
| `NextContractId` | `u32` | `create_contract` |
| `ReputationIssued(id)` | `bool` | `issue_reputation` |
| `PendingReputationCredits(address)` | `u32` | final release, `issue_reputation` |
| `Reputation(address)` | `ReputationRecord` | `issue_reputation` |
| `Finalization(id)` | `FinalizationRecord` | `finalize_contract` |
| `ReadinessChecklist` | `ReadinessChecklist` | initialize and emergency controls |
| `ClientContracts(address)` | `Vec<u32>` | create_contract |
| `FreelancerContracts(address)` | `Vec<u32>` | create_contract |

## Declared But Not Live

These keys are declared in `types.rs` but no public entrypoint currently uses
them as a complete feature:

- `MilestoneApprovals`
- `PendingClientMigration`
- `ProtocolFeeBps`
- `AccumulatedProtocolFees`

Protocol fee implementation is tracked in
[#313](https://github.com/Talenttrust/Talenttrust-Contracts/issues/313) and
[#314](https://github.com/Talenttrust/Talenttrust-Contracts/issues/314).

## Milestone Released State — Single Source of Truth

`release_milestone` sets `milestone.released = true` inside the persisted
`Vec<Milestone>` stored under `(DataKey::Contract(id), milestone_symbol)` where `milestone_symbol` is derived via the centralized `crate::milestone_symbol` helper returning `symbol_short!("milestone")`.

> [!WARNING]
> **Migration Note:** The storage key symbol for milestones was shortened from `"milestones"` to `"milestone"` to allow optimization using compile-time `symbol_short!("milestone")` constants, reducing runtime host-call overhead. Consequently, any milestones persisted under the old `"milestones"` key in older contract versions will not be readable in this version without a storage migration.

`summarize_contract` (called by `finalize_contract`) derives
`released_milestone_count` by iterating that same vector and counting
`ms.released == true`. There is **no** separate `DataKey::MilestoneReleased`
key — that variant was removed in fix [#416] because it was never written,
causing `released_milestone_count` to always report zero in finalization
summaries.

Read and write path are now identical: the milestone vector is the sole
authority for released state.

### 3. Reputation Auditing States
* **`PendingReputation(Address)` / `ReputationIssued(u32)`**
    * **Description:** Bookkeeping indices capturing un-issued tokens and completion certificates for network participants.
    * **Storage Lifespan:** `Persistent`. Preserved explicitly to guarantee deterministic chronological processing when users harvest pending system values.

- Contract ids are monotonically assigned from `NextContractId`.
- Milestone amounts and participant addresses are immutable after creation.
- `funded_amount`, `released_amount`, and `refunded_amount` are checked after
  balance-changing operations.
- `deposit_funds` preserves the aggregate `Contract(id).funded_amount` while
  also allocating each accepted deposit across `(Contract(id), "milestones")`
  in milestone order. A milestone's `funded_amount` is capped at its immutable
  `amount` before funding moves to the next milestone.
- Deposits that would move aggregate funding above the milestone total are
  rejected before any per-milestone allocation is persisted.
- `release_milestone` requires both aggregate availability and the target
  milestone's own `funded_amount >= amount`, so legacy aggregate-only funding
  cannot release an underallocated milestone.
- A milestone release flag can move from absent/false to true only once.
- Reputation issuance is guarded by `ReputationIssued(contract_id)`.

## Read-Only Views and TTL Extensions

### `get_contract_summary(contract_id)`
The `get_contract_summary` entrypoint compiles a read-only `ContractSummary` struct of a contract's metadata and its milestones for off-chain consumers (front-ends and indexers).

To prevent active contract details from expiring and getting archived by the network, querying `get_contract_summary` automatically extends the persistent storage TTL for:
- The contract record (`DataKey::Contract(contract_id)`)
- The milestones vector (`(DataKey::Contract(contract_id), "milestones")`)

This allows off-chain services or users to keep contract storage alive through reads without requiring caller authentication or mutating transaction fees.
## TTL Policy Overview

The escrow contract uses a deterministic TTL model defined in `contracts/escrow/src/ttl.rs`. The key constants and their meanings are:

| Constant | Ledger count | Approx. days | Governs |
|---|---|---|---|
| `LEDGERS_PER_DAY` | 17_280 | 1 | Conversion factor |
| `PENDING_APPROVAL_TTL_LEDGERS` | 120_960 | 7 | Transient approval entries (temporary storage) |
| `PENDING_MIGRATION_TTL_LEDGERS` | 362_880 | 21 | Transient migration entries (temporary storage) |
| `PERSISTENT_TTL_LEDGERS` | 518_400 | 30 | Persistent contract data (persistent storage) |
| `PENDING_APPROVAL_BUMP_THRESHOLD` | 17_280 | 1 | Bump‑on‑read threshold for approvals |
| `PENDING_MIGRATION_BUMP_THRESHOLD` | 51_840 | 3 | Bump‑on‑read threshold for migrations |
| `PERSISTENT_BUMP_THRESHOLD` | 120_960 | 7 | Bump‑on‑read threshold for persistent entries |

**Bump‑on‑read** – When a transient entry is accessed via the contract and its remaining TTL drops below the corresponding bump threshold, the TTL is automatically extended to the full constant value. This keeps active entries alive while allowing stale entries to expire.

**Eviction** – Persistent entries (contracts, milestones, reputation indices) are evicted if they are not accessed for `PERSISTENT_TTL_LEDGERS` (≈30 days). After eviction, reads return `None`, implementing a fail‑closed security posture.

**`read_if_live` semantics** – Returns `None` for both absent keys and keys that have expired and been evicted, ensuring that missing approvals or migrations are treated as invalid.
