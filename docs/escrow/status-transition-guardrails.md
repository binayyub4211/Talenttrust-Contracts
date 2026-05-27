# Escrow Contract Status Transition Guardrails

The escrow contract implements a strict status transition guardrail to avoid invalid workflows, prevent fund stranding, and ensure fail-closed semantics across all state-changing operations.

## Valid status transitions

- `Created` → `Funded` (via `deposit_funds`)
- `Created` → `Cancelled` (via `cancel_contract`)
- `Funded` → `Completed` (via `release_milestone` on final milestone)
- `Funded` → `Disputed` (via `raise_dispute`)
- `Funded` → `PartiallyFunded` → `Cancelled` (via `cancel_contract`)
- `Funded` → `Refunded` (via `refund_unreleased_milestones` on all milestones)
- `Disputed` → `Completed` (via `finalize_contract`)

## Operation-specific state requirements

### `deposit_funds`
- Requires: `Created` or `PartiallyFunded`
- Transitions to: `Funded` (if all required funds deposited) or remains `PartiallyFunded`
- Effect: Cannot deposit into terminal or in-resolution states

### `release_milestone`
- Requires: `Funded`
- Transitions to: `Completed` (if final milestone released) or remains `Funded`
- Effect: Prevents release from disputed or refunded contracts

### `cancel_contract` (NEW: State Guardrails)
- **Allowed from**: `Created`, `PartiallyFunded`, `Funded`
- **Blocked from**: `Disputed`, `Refunded`, `Completed`, `Cancelled`
- Authorization: `client` or `freelancer` only (arbiter cannot cancel)
- Invariant: No-op-or-error from terminal/in-resolution states
- Purpose: Prevent fund stranding or double-resolution once dispute is raised or funds refunded

### `dispute_contract` (via `raise_dispute`)
- Requires: `Funded`
- Transitions to: `Disputed`
- Effect: Blocks milestone release; requires explicit resolution

### `refund_unreleased_milestones`
- Requires: `Funded`
- Transitions to: `Refunded` (if all milestones refunded) or remains `Funded`
- Effect: Partial refunds leave in `Funded`, full refund transitions to `Refunded`

### `finalize_contract`
- Requires: `Completed` or `Disputed`
- Effect: Records immutable closure metadata; no further transitions allowed

## Security properties

- **Fail-closed state machine**: Invalid transitions panic immediately during contract creation and at each mutation point.
- **Authorization checks**: All state-changing operations enforce caller role verification before applying state changes.
- **Accounting invariants**: All mutations include `check_accounting_invariant` to detect fund loss or stranding.
- **Terminal states**: Once a contract reaches `Cancelled`, `Completed`, or `Refunded`, no further state mutations are possible.
- **Dispute resolution**: Disputed contracts require explicit finalization by contract parties; unilateral cancellation is blocked.

## Cancel Contract State Guardrails (Security Fix)

The `cancel_contract` function enforces strict source-state restrictions to prevent:

1. **Fund stranding**: Cancellation from `Disputed` or `Refunded` could leave funds locked or cause double-refund.
2. **Unilateral bypass of dispute**: Once disputed, neither party can cancel unilaterally; arbiter resolution is required.
3. **Accounting violations**: Double-release or refund scenarios that violate the invariant `total_deposited ≥ released + refunded`.

### Cancellable states (pre-resolution, non-terminal)
- `Created`: No funds deposited; safe to cancel.
- `PartiallyFunded`: Some funds deposited; refund accounting is simple (no milestones released).
- `Funded`: All funds deposited but no releases yet; cancel is the economic deterrent before work begins.

### Rejected states (terminal or in-resolution)
- `Disputed`: Active dispute; requires arbiter resolution or finalization, not unilateral cancellation.
- `Refunded`: Already refunded; cancellation would violate refund finality.
- `Completed`: Contract complete; terminal state, no mutations allowed.
- `Cancelled`: Already cancelled; rejected with `AlreadyCancelled` rather than `InvalidStatusTransition`.

### Authorization model
- **Client** and **freelancer** can cancel from allowed states (economic deterrent).
- **Arbiter** cannot cancel even in allowed states (arbiter role is dispute resolution only).
- Unauthorized callers are rejected with `UnauthorizedRole`.

## Testing strategy

### Valid cancellation scenarios
- Client cancels from `Created`, `PartiallyFunded`, `Funded`
- Freelancer cancels from allowed states
- Events are emitted with correct state transitions

### Invalid cancellation scenarios (should panic with `InvalidStatusTransition`)
- Client/freelancer attempt to cancel from `Disputed`
- Client/freelancer attempt to cancel from `Refunded`
- Arbiter attempts to cancel from any state (unauthorized role)
- Double cancellation (second attempt fails with `AlreadyCancelled`)

### Security invariants
- Accounting invariant is enforced on every transition
- Storage TTL and fee accounting are respected
- No fund loss or stranding is possible from any panic path
