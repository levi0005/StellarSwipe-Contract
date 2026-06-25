# Contributing to StellarSwipe-Contract

## Scaffold a new contract crate

Use the scaffold generator to create a new contract crate already wired with the
shared **Pausable**, **Initializable**, and **StorageTrait** conventions:

```bash
# From the repository root
./stellar-swipe/scripts/scaffold_contract.sh <contract_name>
```

### Example

```bash
./stellar-swipe/scripts/scaffold_contract.sh reward_distributor
```

This creates:

```
stellar-swipe/contracts/reward_distributor/
├── Cargo.toml          # depends on soroban-sdk + stellar-swipe-common
└── src/
    ├── lib.rs          # initialize / pause / unpause / storage_write / storage_read
    └── tests.rs        # starter tests covering init, pause, storage round-trip
```

The workspace `Cargo.toml` is updated automatically to include the new crate.

### Verify the scaffold

```bash
cd stellar-swipe
cargo test   -p stellar-swipe-reward-distributor
cargo clippy -p stellar-swipe-reward-distributor -- -D warnings
```

Both should pass with no manual fixes required.

### What the scaffold includes

| Feature | Implementation |
|---|---|
| Initializable guard | `initialize()` panics/returns error on double-init |
| Pausable | `pause()` / `unpause()` / `is_paused()` with events |
| StorageTrait pattern | `storage_write(key, value)` / `storage_read(key)` blocked while paused |
| Starter test file | `tests.rs` with 5 tests covering all three features |

Extend `DataKey` and `{ContractName}Error` with your contract-specific variants
before adding business logic.
