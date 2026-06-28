#!/usr/bin/env python3
"""
check_wasm_exports.py — detect breaking changes to Soroban contract ABI exports.

Usage:
    python3 scripts/check_wasm_exports.py [--wasm-dir <dir>]

    The script is run automatically in CI after the optimized WASM build step.
    It can also be run locally against any directory of *.wasm files.

Exit codes:
    0  All contracts match their committed baselines (or only gained exports).
    1  A breaking change was detected (removed or changed export) without an
       explicit acknowledgement file.
    2  New exports were found; baselines updated. Commit the updated JSON files.

What counts as a breaking change:
    - A function name present in the baseline is absent from the current WASM.
    - The Soroban contract-spec (contractspecv0 custom section) hash changed,
      indicating a parameter name / type / order change in an existing function.

Non-breaking:
    - New exports added since the last baseline snapshot.

Acknowledging a deliberate breaking change:
    Create the file  abi-baselines/<contract-name>.breaking.txt  in the repo.
    Its presence tells this script that the breaking change is intentional and
    paired with a migration / major-version bump.
    The script will still update the baseline so the next run passes.

Baseline files:
    abi-baselines/<contract-name>.json  — committed JSON describing the ABI.

    Format:
    {
      "contract": "<name>",
      "exports": ["fn1", "fn2", ...],
      "spec_hash": "<sha256 of raw contractspecv0 section bytes, or empty>"
    }
"""

import argparse
import hashlib
import json
import os
import struct
import sys
from pathlib import Path
from typing import Dict, List, Optional, Tuple

WORKSPACE_ROOT = Path(__file__).resolve().parent.parent
WASM_DIR_DEFAULT = WORKSPACE_ROOT / "target" / "wasm-optimized"
BASELINES_DIR = WORKSPACE_ROOT / "abi-baselines"


# ── WASM binary helpers ────────────────────────────────────────────────────────

def _read_leb128(data: bytes, pos: int) -> Tuple[int, int]:
    """Read an unsigned LEB128 integer from data[pos:].  Returns (value, bytes_consumed)."""
    result = 0
    shift = 0
    consumed = 0
    while True:
        b = data[pos + consumed]
        consumed += 1
        result |= (b & 0x7F) << shift
        shift += 7
        if (b & 0x80) == 0:
            break
    return result, consumed


def _parse_wasm_sections(data: bytes) -> Dict[int, bytes]:
    """Return {section_id: raw_section_bytes} for every section in the WASM binary."""
    if data[:4] != b"\x00asm":
        raise ValueError("Not a valid WASM binary (bad magic)")
    if data[4:8] != b"\x01\x00\x00\x00":
        raise ValueError("Unsupported WASM version")

    sections: Dict[int, bytes] = {}
    pos = 8
    while pos < len(data):
        section_id = data[pos]
        pos += 1
        size, n = _read_leb128(data, pos)
        pos += n
        sections[section_id] = data[pos : pos + size]
        pos += size
    return sections


def extract_export_names(wasm_data: bytes) -> List[str]:
    """Return all function export names from the WASM export section (id=7)."""
    try:
        sections = _parse_wasm_sections(wasm_data)
    except ValueError:
        return []

    exports_raw = sections.get(7)
    if not exports_raw:
        return []

    pos = 0
    count, n = _read_leb128(exports_raw, pos)
    pos += n
    names: List[str] = []
    for _ in range(count):
        name_len, n = _read_leb128(exports_raw, pos)
        pos += n
        name = exports_raw[pos : pos + name_len].decode("utf-8", errors="replace")
        pos += name_len
        export_kind = exports_raw[pos]
        pos += 1
        _index, n = _read_leb128(exports_raw, pos)
        pos += n
        if export_kind == 0x00:  # 0x00 = function
            names.append(name)
    return names


def extract_spec_hash(wasm_data: bytes) -> str:
    """Return SHA-256 of the raw `contractspecv0` custom section bytes, or '' if absent.

    The Soroban host embeds the contract spec (function signatures, types) in a
    custom WASM section named 'contractspecv0'.  Hashing the raw bytes gives a
    stable fingerprint: any change to parameter names, types, or order changes
    the hash, flagging a potential ABI break.
    """
    try:
        sections = _parse_wasm_sections(wasm_data)
    except ValueError:
        return ""

    custom_section_id = 0
    raw = sections.get(custom_section_id)
    if not raw:
        return ""

    # The custom section can appear multiple times in a WASM binary; we need to
    # re-parse from the raw binary to collect all of them (the dict above only
    # keeps the last one for each id).  Scan the binary for custom sections.
    custom_payloads: List[bytes] = []
    data = wasm_data
    pos = 8
    while pos < len(data):
        section_id = data[pos]
        pos += 1
        size, n = _read_leb128(data, pos)
        pos += n
        section_body = data[pos : pos + size]
        pos += size
        if section_id == 0x00:  # custom section
            # First field is the name (LEB128 length + UTF-8 bytes)
            name_len, nb = _read_leb128(section_body, 0)
            name = section_body[nb : nb + name_len].decode("utf-8", errors="replace")
            if name == "contractspecv0":
                payload = section_body[nb + name_len :]
                custom_payloads.append(payload)

    if not custom_payloads:
        return ""

    h = hashlib.sha256()
    for p in custom_payloads:
        h.update(p)
    return h.hexdigest()


# ── Baseline I/O ───────────────────────────────────────────────────────────────

def load_baseline(contract_name: str) -> Optional[dict]:
    path = BASELINES_DIR / f"{contract_name}.json"
    if not path.exists():
        return None
    return json.loads(path.read_text())


def save_baseline(contract_name: str, data: dict) -> None:
    BASELINES_DIR.mkdir(parents=True, exist_ok=True)
    path = BASELINES_DIR / f"{contract_name}.json"
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def has_breaking_ack(contract_name: str) -> bool:
    """Return True if a deliberate-breaking-change acknowledgement file exists."""
    return (BASELINES_DIR / f"{contract_name}.breaking.txt").exists()


# ── Core comparison logic ──────────────────────────────────────────────────────

def check_contract(wasm_path: Path) -> Tuple[bool, bool]:
    """Check one WASM file against its baseline.

    Returns:
        (has_breaking_change, baseline_updated)
    """
    contract_name = wasm_path.stem  # e.g. "signal_registry"
    wasm_data = wasm_path.read_bytes()

    current_exports = sorted(extract_export_names(wasm_data))
    current_spec_hash = extract_spec_hash(wasm_data)

    current = {
        "contract": contract_name,
        "exports": current_exports,
        "spec_hash": current_spec_hash,
    }

    baseline = load_baseline(contract_name)
    if baseline is None:
        # First run — generate the baseline.
        save_baseline(contract_name, current)
        print(
            f"  [{contract_name}] No baseline found — created initial snapshot "
            f"({len(current_exports)} exports). Commit abi-baselines/{contract_name}.json."
        )
        return False, True

    baseline_exports = set(baseline.get("exports", []))
    current_exports_set = set(current_exports)
    baseline_spec_hash = baseline.get("spec_hash", "")

    removed = baseline_exports - current_exports_set
    added = current_exports_set - baseline_exports
    spec_changed = (
        current_spec_hash != baseline_spec_hash
        and baseline_spec_hash != ""
        and current_spec_hash != ""
    )

    breaking = bool(removed) or spec_changed

    if breaking:
        if has_breaking_ack(contract_name):
            print(
                f"  [{contract_name}] Breaking change ACKNOWLEDGED "
                f"(abi-baselines/{contract_name}.breaking.txt present)."
            )
            if removed:
                print(f"    Removed exports: {sorted(removed)}")
            if spec_changed:
                print(
                    f"    Spec hash changed: {baseline_spec_hash[:16]}... "
                    f"→ {current_spec_hash[:16]}..."
                )
            # Update baseline to reflect intentional change.
            save_baseline(contract_name, current)
            return False, True
        else:
            print(f"  [{contract_name}] BREAKING CHANGE DETECTED:", file=sys.stderr)
            if removed:
                print(
                    f"    Removed exports (callers will break): {sorted(removed)}",
                    file=sys.stderr,
                )
            if spec_changed:
                print(
                    f"    Contract spec hash changed (parameter signatures may have "
                    f"changed):\n"
                    f"      baseline : {baseline_spec_hash[:32]}...\n"
                    f"      current  : {current_spec_hash[:32]}...",
                    file=sys.stderr,
                )
            print(
                f"    To acknowledge this deliberate breaking change, create:\n"
                f"      abi-baselines/{contract_name}.breaking.txt\n"
                f"    and ensure a migration / major-version bump accompanies it.",
                file=sys.stderr,
            )
            return True, False

    # Non-breaking — may have new exports.
    updated = False
    if added:
        print(f"  [{contract_name}] New exports added: {sorted(added)} — updating baseline.")
        save_baseline(contract_name, current)
        updated = True
    else:
        print(f"  [{contract_name}] OK ({len(current_exports)} exports, spec hash matches).")

    return False, updated


# ── Entry point ────────────────────────────────────────────────────────────────

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--wasm-dir",
        default=str(WASM_DIR_DEFAULT),
        help="Directory containing optimized *.wasm files (default: target/wasm-optimized/)",
    )
    args = parser.parse_args()

    wasm_dir = Path(args.wasm_dir)
    if not wasm_dir.is_dir():
        print(
            f"WASM directory not found: {wasm_dir}\n"
            f"Build the contracts first: cd stellar-swipe && ./scripts/build.sh",
            file=sys.stderr,
        )
        return 1

    wasm_files = sorted(wasm_dir.glob("*.wasm"))
    if not wasm_files:
        print(f"No *.wasm files found in {wasm_dir}", file=sys.stderr)
        return 1

    print(f"Checking {len(wasm_files)} WASM contract(s) in {wasm_dir}:")

    any_breaking = False
    any_updated = False

    for wasm_path in wasm_files:
        breaking, updated = check_contract(wasm_path)
        any_breaking = any_breaking or breaking
        any_updated = any_updated or updated

    print()
    if any_breaking:
        print(
            "RESULT: One or more breaking ABI changes detected without acknowledgement.\n"
            "        See errors above. Fix or acknowledge before merging.",
            file=sys.stderr,
        )
        return 1

    if any_updated:
        print(
            "RESULT: Baselines updated for new or intentionally changed exports.\n"
            "        Commit the updated abi-baselines/*.json files."
        )
        return 2

    print("RESULT: All contract ABIs match their committed baselines.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
