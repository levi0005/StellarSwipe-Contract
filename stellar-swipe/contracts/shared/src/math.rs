//! Decimal-precision scaling helpers (Issue #562).
//!
//! Provides checked functions to convert an `i128` amount between two
//! arbitrary decimal precisions.  A single canonical implementation lives
//! here so oracle conversion, trade execution, and any other module that
//! needs to rescale amounts all use the same rounding behavior.
//!
//! # Rounding
//! Scale-down (from higher to lower precision) **truncates** toward zero
//! (integer division).  This is explicit, not implicit: callers that need
//! rounding must apply it on top of the raw result.
//!
//! # Overflow / invalid inputs
//! All functions return `None` on overflow.  `from_decimals` and
//! `to_decimals` are `u32`, so negative precision is a type-level
//! impossibility.  Precision values large enough to overflow `i128` (i.e.
//! those that would multiply by 10^38 or more) also return `None`.

/// Convert `amount` from `from_decimals` precision to `to_decimals` precision.
///
/// Rounding: scale-down truncates toward zero.
/// Returns `None` on arithmetic overflow or if the scale factor overflows
/// `i128` (e.g. `to_decimals - from_decimals >= 39`).
///
/// # Examples
/// ```ignore
/// // Same precision — no change.
/// assert_eq!(normalize_amount(10_000_000, 7, 7), Some(10_000_000));
/// // 6-decimal → 7-decimal: ×10
/// assert_eq!(normalize_amount(1_000_000, 6, 7), Some(10_000_000));
/// // 7-decimal → 6-decimal: ÷10 (truncates)
/// assert_eq!(normalize_amount(10_000_001, 7, 6), Some(1_000_000));
/// ```
pub fn normalize_amount(amount: i128, from_decimals: u32, to_decimals: u32) -> Option<i128> {
    match from_decimals.cmp(&to_decimals) {
        core::cmp::Ordering::Equal => Some(amount),
        core::cmp::Ordering::Less => {
            let diff = to_decimals - from_decimals;
            let factor = pow10(diff)?;
            amount.checked_mul(factor)
        }
        core::cmp::Ordering::Greater => {
            let diff = from_decimals - to_decimals;
            let factor = pow10(diff)?;
            Some(amount / factor)
        }
    }
}

/// Scale `amount` up from `from_decimals` to `to_decimals` precision.
///
/// Panics (via `None`) if `to_decimals < from_decimals`; use
/// [`normalize_amount`] for the general case.
///
/// Returns `None` on overflow.
#[inline]
pub fn scale_up(amount: i128, from_decimals: u32, to_decimals: u32) -> Option<i128> {
    normalize_amount(amount, from_decimals, to_decimals)
}

/// Scale `amount` down from `from_decimals` to `to_decimals` precision,
/// truncating toward zero.
///
/// Returns `None` if `to_decimals > from_decimals`; use [`normalize_amount`]
/// for the general case.
#[inline]
pub fn scale_down(amount: i128, from_decimals: u32, to_decimals: u32) -> Option<i128> {
    normalize_amount(amount, from_decimals, to_decimals)
}

/// Compute 10^exp as i128. Returns `None` if the result overflows.
fn pow10(exp: u32) -> Option<i128> {
    let mut result: i128 = 1;
    for _ in 0..exp {
        result = result.checked_mul(10)?;
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_precision_unchanged() {
        assert_eq!(normalize_amount(10_000_000, 7, 7), Some(10_000_000));
        assert_eq!(normalize_amount(0, 7, 7), Some(0));
    }

    #[test]
    fn lower_to_higher_precision() {
        assert_eq!(normalize_amount(1_000_000, 6, 7), Some(10_000_000));
        assert_eq!(normalize_amount(1, 0, 7), Some(10_000_000));
    }

    #[test]
    fn higher_to_lower_precision_truncates() {
        assert_eq!(normalize_amount(10_000_000, 7, 6), Some(1_000_000));
        assert_eq!(normalize_amount(10_000_001, 7, 6), Some(1_000_000)); // truncated
        assert_eq!(normalize_amount(10_000_000, 7, 0), Some(1));
    }

    #[test]
    fn overflow_returns_none() {
        assert_eq!(normalize_amount(i128::MAX, 0, 39), None);
    }

    #[test]
    fn negative_amounts_work() {
        assert_eq!(normalize_amount(-1_000_000, 6, 7), Some(-10_000_000));
        assert_eq!(normalize_amount(-10_000_000, 7, 6), Some(-1_000_000));
    }

    /// Round-trip: scale up then down must recover the original value
    /// within the truncation tolerance (loss ≤ 10^diff - 1).
    #[test]
    fn round_trip_scale_up_then_down() {
        let amounts: &[i128] = &[0, 1, 999_999, 1_000_000, i128::MAX / 100];
        for &amount in amounts {
            let scaled = normalize_amount(amount, 6, 7).unwrap();
            let recovered = normalize_amount(scaled, 7, 6).unwrap();
            assert_eq!(recovered, amount, "round-trip failed for {}", amount);
        }
    }

    /// Round-trip starting from 7-decimal: extra sub-unit may be lost on
    /// scale-down, but recovered value must equal original / 10 * 10.
    #[test]
    fn round_trip_scale_down_then_up() {
        let amounts: &[i128] = &[10_000_000, 10_000_009, 99_999_999];
        for &amount in amounts {
            let down = normalize_amount(amount, 7, 6).unwrap();
            let up = normalize_amount(down, 6, 7).unwrap();
            // Rounding loss ≤ 9 (one sub-unit at 7 decimals)
            assert!(
                amount - up < 10 && amount >= up,
                "round-trip tolerance exceeded for {}",
                amount
            );
        }
    }
}
