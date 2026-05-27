# PR Request: Add Two-Step Governance Admin Transfer for Escrow

## Summary
This PR adds a secure two-step admin transfer mechanism to the TalentTrust escrow governance layer in `contracts/escrow`.

## What changed
- Implemented governance storage and management in `contracts/escrow/src/governance.rs`
- Added protocol parameter support and governance state keys in `contracts/escrow/src/types.rs`
- Updated `contracts/escrow/src/lib.rs` to expose governance functions and types
- Added/updated governance tests in `contracts/escrow/src/test/governance.rs`
- Clarified governance documentation in `docs/escrow/governance-security.md`

## New public governance behavior
- `initialize_protocol_governance(admin, ...) -> bool`
- `initialize_governance(admin) -> bool`
- `update_protocol_parameters(...) -> bool`
- `propose_governance_admin(new_admin) -> bool`
- `accept_governance_admin() -> bool`
- `get_protocol_parameters() -> ProtocolParameters`
- `get_governance_admin() -> Option<Address>`
- `get_pending_governance_admin() -> Option<Address>`

## Security model enforced
- Current governance admin must authenticate to propose a new admin
- Proposed admin must authenticate to accept transfer
- Pending admin state is cleared on acceptance
- A new proposal overwrites any existing pending admin
- Current admin retains control until the transfer is accepted

## Files changed
- `contracts/escrow/src/types.rs`
- `contracts/escrow/src/lib.rs`
- `contracts/escrow/src/governance.rs`
- `contracts/escrow/src/test/governance.rs`
- `contracts/escrow/src/test/mainnet_readiness.rs`
- `docs/escrow/governance-security.md`

## Test plan
Run locally in the repo root:

```bash
cargo fmt --all
cargo test -p escrow
cargo test -p escrow governance
cargo test test::performance -p escrow
cargo fmt --all -- --check
```

## Notes
- This PR targets the escrow governance layer only.
- The implementation is intentionally scoped to the governance admin transfer flow and its supporting protocol parameter state.
- The document is written for reviewers and release notes.
