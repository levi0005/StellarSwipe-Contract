//! Hot-entry TTL management for persistent storage.
//!
//! Soroban archives persistent entries whose TTL reaches zero.  For keys that
//! are read frequently (active signals, leaderboard indexes) we extend their
//! TTL on every access **only when the remaining TTL has dropped below the
//! configured threshold**, avoiding wasteful extend calls on every single read.
//!
//! # Constants
//! | Constant | Default | Purpose |
//! |---|---|---|
//! | `HOT_KEY_TTL_TARGET_LEDGERS` | 518 400 (~30 days) | Desired TTL after a bump |
//! | `HOT_KEY_TTL_THRESHOLD_LEDGERS` | 103 680 (~6 days) | Bump only when TTL falls below this |

use soroban_sdk::{Env, IntoVal, Val};

/// Target TTL (ledgers) after an extend: ~30 days at 5-second ledger close.
pub const HOT_KEY_TTL_TARGET_LEDGERS: u32 = 518_400;

/// Only extend when the remaining TTL drops below this threshold: ~6 days.
/// Prevents extending on every single access while still keeping the entry
/// alive well before it would be archived.
pub const HOT_KEY_TTL_THRESHOLD_LEDGERS: u32 = 103_680;

/// Extend the TTL of a **persistent** storage entry identified by `key` if its
/// remaining TTL is below [`HOT_KEY_TTL_THRESHOLD_LEDGERS`].
///
/// Does nothing when:
/// - The entry does not exist in persistent storage.
/// - The remaining TTL is already above the threshold (avoids wasted instructions).
pub fn bump_persistent_if_needed<K>(env: &Env, key: &K)
where
    K: IntoVal<Env, Val>,
{
    let storage = env.storage().persistent();
    if !storage.has(key) {
        return;
    }
    let current_ttl = storage.get_ttl(key);
    if current_ttl < HOT_KEY_TTL_THRESHOLD_LEDGERS {
        storage.extend_ttl(key, HOT_KEY_TTL_THRESHOLD_LEDGERS, HOT_KEY_TTL_TARGET_LEDGERS);
    }
}

/// Unconditionally extend the TTL of a persistent entry to
/// [`HOT_KEY_TTL_TARGET_LEDGERS`] regardless of the current remaining TTL.
///
/// Use this in the keeper batch-bump entrypoint where the caller explicitly
/// wants to top-up a set of keys.
pub fn force_bump_persistent<K>(env: &Env, key: &K)
where
    K: IntoVal<Env, Val>,
{
    let storage = env.storage().persistent();
    if storage.has(key) {
        storage.extend_ttl(key, 0, HOT_KEY_TTL_TARGET_LEDGERS);
    }
}

#[cfg(any(test, feature = "testutils"))]
pub mod testutils {
    use super::*;

    /// Returns the current TTL for a persistent key, or 0 if absent.
    pub fn get_ttl_or_zero<K>(env: &Env, key: &K) -> u32
    where
        K: IntoVal<Env, Val>,
    {
        let storage = env.storage().persistent();
        if storage.has(key) {
            storage.get_ttl(key)
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, contracttype, Env};

    #[contracttype]
    #[derive(Clone)]
    enum TestKey {
        Hot,
        Missing,
    }

    #[contract]
    struct TtlHarness;

    #[contractimpl]
    impl TtlHarness {}

    fn setup() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        let id = env.register(TtlHarness, ());
        (env, id)
    }

    #[test]
    fn bump_extends_ttl_for_existing_entry() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            env.storage().persistent().set(&TestKey::Hot, &42u32);
            // Initial TTL should be at the protocol minimum (1 ledger after set in testenv).
            // Force it below the threshold so bump_persistent_if_needed triggers.
            env.storage()
                .persistent()
                .extend_ttl(&TestKey::Hot, 0, HOT_KEY_TTL_THRESHOLD_LEDGERS - 1);

            let ttl_before = env.storage().persistent().get_ttl(&TestKey::Hot);
            bump_persistent_if_needed(&env, &TestKey::Hot);
            let ttl_after = env.storage().persistent().get_ttl(&TestKey::Hot);
            assert!(ttl_after > ttl_before, "TTL should increase after bump");
            assert!(ttl_after >= HOT_KEY_TTL_THRESHOLD_LEDGERS);
        });
    }

    #[test]
    fn bump_skips_when_ttl_above_threshold() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            env.storage().persistent().set(&TestKey::Hot, &99u32);
            env.storage()
                .persistent()
                .extend_ttl(&TestKey::Hot, 0, HOT_KEY_TTL_TARGET_LEDGERS);

            let ttl_before = env.storage().persistent().get_ttl(&TestKey::Hot);
            bump_persistent_if_needed(&env, &TestKey::Hot);
            let ttl_after = env.storage().persistent().get_ttl(&TestKey::Hot);
            // TTL must not decrease (no unnecessary extend).
            assert!(ttl_after >= ttl_before, "TTL must not decrease on no-op bump");
        });
    }

    #[test]
    fn bump_noop_for_missing_entry() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            // Must not panic.
            bump_persistent_if_needed(&env, &TestKey::Missing);
        });
    }

    #[test]
    fn force_bump_always_extends() {
        let (env, id) = setup();
        env.as_contract(&id, || {
            env.storage().persistent().set(&TestKey::Hot, &1u32);
            env.storage()
                .persistent()
                .extend_ttl(&TestKey::Hot, 0, HOT_KEY_TTL_TARGET_LEDGERS);
            let ttl_before = env.storage().persistent().get_ttl(&TestKey::Hot);
            force_bump_persistent(&env, &TestKey::Hot);
            let ttl_after = env.storage().persistent().get_ttl(&TestKey::Hot);
            assert!(
                ttl_after >= ttl_before,
                "force_bump must not shrink TTL from target"
            );
        });
    }
}
