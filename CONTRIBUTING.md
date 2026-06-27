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

## Dependency policy

CI runs `cargo deny check` (via `stellar-swipe/deny.toml`) on every PR that
touches a `Cargo.toml` or `Cargo.lock`.  The policy covers:

| Area | Rule |
|------|------|
| Licenses | Only the allow-list in `deny.toml` is permitted.  Unlicensed or copyleft crates are denied. |
| Banned crates | `openssl` and `ring ≤0.16` are banned.  See `[bans]` for the full list. |
| Git sources | `unknown-git = "deny"`.  Git dependencies pinned to a mutable branch (no `rev =`) are not allowed; they break reproducible builds. |
| Advisories | Crates with active RustSec advisories are denied unless individually listed in `[advisories] ignore` with a documented reason. |

### Adding a new dependency

1. Add the dependency to the relevant `Cargo.toml`.
2. Run `cargo deny check` locally (`cargo install cargo-deny` if not installed).
3. If the check passes, open a normal PR.
4. If the check fails (e.g. the crate uses a license not on the allow-list):
   - Replace the dependency with a compliant alternative, **or**
   - Open a PR with the `dependency-review` label and explain why the policy
     should be extended.  A maintainer must approve before the dependency can
     be added.

### Requesting a policy exception

Open a PR with the `dependency-review` label.  The description must include:

1. The crate name, version, and why it is needed.
2. Why a compliant alternative does not exist.
3. The specific `deny.toml` change required (e.g. adding a license or an
   advisory to the ignore list).
4. For advisories: the upstream issue/PR tracking the fix.

Two maintainer approvals are required for any policy exception.

## Clippy policy

CI runs `cargo clippy --workspace --all-targets -- -D warnings`.  Any clippy
warning fails the build.

### Suppressing a lint

- **Prefer fixing** the underlying issue over suppressing.
- Suppressions must be **as narrow as possible**: annotate the individual item
  (`fn`, `impl` block, expression) rather than the whole module or crate.
- The `#[allow]` attribute must include a brief comment explaining why the
  suppression is justified:

  ```rust
  // Soroban contract functions cannot use struct wrappers in the public ABI.
  #[allow(clippy::too_many_arguments)]
  pub fn my_contract_fn(env: Env, a: Address, …) { … }
  ```

- Workspace-wide suppressions live in `[workspace.lints.clippy]` in
  `stellar-swipe/Cargo.toml` and require a PR that explains why the lint is
  non-actionable across the whole workspace.

### Requesting an exception

Open a PR with the `lint-exception` label.  The PR description must include:

1. The lint name and the code it fires on.
2. Why fixing the code is not preferable.
3. A `#[allow]` annotation scoped to the narrowest applicable span.

Reviewers will merge only after confirming the suppression scope is minimal.
