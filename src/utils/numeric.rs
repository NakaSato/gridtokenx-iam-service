//! Robust numeric conversion and safe arithmetic for the IAM Service
//! 
//! This module provides safe helpers for:
//! - Converting Decimal to Atomic Unit (u64) for internal financial logic
//! - Safe casting for user balances (20,8 precision)
//! - Context-enriched errors for balance/financial updates

use anyhow::{Context, Result, anyhow};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

/// Converts a Decimal value to a u64 in atomic units (e.g., 8 decimals for internal balance representation).
/// 
/// Returns an error if:
/// - The value would overflow a u64
/// - The value is negative (balances must generally be positive)
/// - Precision loss would occur (input has more than `decimals` significant fractional digits)
pub fn to_u64_atomic(val: Decimal, decimals: u32, label: &str) -> Result<u64> {
    if val.is_sign_negative() {
        return Err(anyhow!("Negative value for {}: {}", label, val));
    }

    let multiplier = Decimal::from(10u64.pow(decimals));
    let atomic_val = val * multiplier;

    // Ensure it's an integer (no fractional parts after scaling)
    if atomic_val.fract() != Decimal::ZERO {
        return Err(anyhow!(
            "Precision loss for {}: {} with {} decimals would lose fractional digits",
            label, val, decimals
        ));
    }

    atomic_val.to_u64().context(format!(
        "Value too large for u64 after scaling {}: {} (scaled: {})", 
        label, val, atomic_val
    ))
}

/// Safely divides two Decimal values, returning an error instead of panic/NaN for zero denominators.
pub fn safe_div(numerator: Decimal, denominator: Decimal, label: &str) -> Result<Decimal> {
    if denominator.is_zero() {
        return Err(anyhow!("Division by zero for {}: {} / {}", label, numerator, denominator));
    }
    Ok(numerator / denominator)
}
