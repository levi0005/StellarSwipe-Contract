# Storage Layout Snapshots

Committed XDR byte baselines for key `#[contracttype]` types used in persistent
storage. These baselines catch silent breaking changes (field reordering, type
changes) that would corrupt already-stored data or break external clients.

## How it works

The Rust test module `signal_registry::storage_layout_tests` serialises a
representative instance of each type into the Soroban host's XDR encoding,
hex-encodes the result, and compares it against the baseline files here.

## Updating a snapshot intentionally

A breaking storage change **must** be accompanied by a migration. Once the
migration is in place:

1. Delete or truncate the relevant `.hex` file.
2. Re-run `cargo test -- storage_layout_tests` — the test will generate the
   new baseline and print it to stdout (look for `SNAPSHOT_UPDATE`).
3. Paste the printed hex into the `.hex` file and commit it alongside the
   migration code.

CI will fail if a layout changes without an updated baseline.

## Files

| File                   | Type            | Contract          |
|------------------------|-----------------|-------------------|
| `signal_data_v1.hex`   | `SignalDataV1`  | signal_registry   |
| `signal_data_v2.hex`   | `SignalDataV2`  | signal_registry   |
| `signal.hex`           | `Signal`        | signal_registry   |
| `scheduled_signal.hex` | `ScheduledSignal` | signal_registry |
