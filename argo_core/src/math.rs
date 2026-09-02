//! Widening multiply/divide with explicit rounding. All arithmetic is checked;
//! an overflow returns `None` and the caller must treat it as a program error.

/// Multiply `x * y` then divide by `d`, rounding down. `None` on overflow or `d == 0`.
pub fn mul_div_down(x: u128, y: u128, d: u128) -> Option<u128> {
    if d == 0 {
        return None;
    }
    let (hi, lo) = widening_mul(x, y);
    div_256_by_128(hi, lo, d)
}

/// Multiply `x * y` then divide by `d`, rounding up. `None` on overflow or `d == 0`.
pub fn mul_div_up(x: u128, y: u128, d: u128) -> Option<u128> {
    if d == 0 {
        return None;
    }
    let (hi, lo) = widening_mul(x, y);
    let q = div_256_by_128(hi, lo, d)?;
    // remainder = (x*y) - q*d, computed in 256 bits to avoid overflow
    let (qh, ql) = widening_mul(q, d);
    let exact = qh == hi && ql == lo;
    if exact {
        Some(q)
    } else {
        q.checked_add(1)
    }
}

/// `x * y / WAD`, rounding down.
pub fn w_mul_down(x: u128, y: u128) -> Option<u128> {
    mul_div_down(x, y, crate::WAD)
}

/// `x * WAD / y`, rounding down.
pub fn w_div_down(x: u128, y: u128) -> Option<u128> {
    mul_div_down(x, crate::WAD, y)
}

/// `x * WAD / y`, rounding up.
pub fn w_div_up(x: u128, y: u128) -> Option<u128> {
    mul_div_up(x, crate::WAD, y)
}

/// Full 128×128 → 256-bit product as `(hi, lo)`.
fn widening_mul(x: u128, y: u128) -> (u128, u128) {
    let (x0, x1) = (x & u64::MAX as u128, x >> 64);
    let (y0, y1) = (y & u64::MAX as u128, y >> 64);
    let p00 = x0 * y0;
    let p01 = x0 * y1;
    let p10 = x1 * y0;
    let p11 = x1 * y1;
    let mid = (p00 >> 64) + (p01 & u64::MAX as u128) + (p10 & u64::MAX as u128);
    let lo = (mid << 64) | (p00 & u64::MAX as u128);
    let hi = p11 + (p01 >> 64) + (p10 >> 64) + (mid >> 64);
    (hi, lo)
}

/// Divide a 256-bit value `(hi, lo)` by a 128-bit `d`. `None` if the quotient
/// does not fit in 128 bits.
fn div_256_by_128(hi: u128, lo: u128, d: u128) -> Option<u128> {
    if hi >= d {
        return None;
    }
    if hi == 0 {
        return Some(lo / d);
    }
    // Bitwise long division; 256 iterations max, cheap in the zkVM relative to
    // the account hashing that dominates a transaction.
    let mut rem: u128 = hi;
    let mut q: u128 = 0;
    let mut i = 128;
    while i > 0 {
        i -= 1;
        let bit = (lo >> i) & 1;
        let carry = rem >> 127;
        rem = (rem << 1) | bit;
        q <<= 1;
        if carry == 1 || rem >= d {
            rem = rem.wrapping_sub(d);
            q |= 1;
        }
    }
    Some(q)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn small_values_match_native() {
        assert_eq!(mul_div_down(10, 3, 4), Some(7));
        assert_eq!(mul_div_up(10, 3, 4), Some(8));
        assert_eq!(mul_div_down(u128::MAX, 1, 1), Some(u128::MAX));
        assert_eq!(mul_div_down(u128::MAX, 2, 1), None);
        assert_eq!(mul_div_down(1, 1, 0), None);
    }

    proptest! {
        #[test]
        fn matches_native_when_product_fits(x in 0u128..(1u128 << 64), y in 0u128..(1u128 << 64), d in 1u128..u128::MAX) {
            let p = x * y;
            prop_assert_eq!(mul_div_down(x, y, d), Some(p / d));
            let up = p / d + u128::from(p % d != 0);
            prop_assert_eq!(mul_div_up(x, y, d), Some(up));
        }

        #[test]
        fn up_is_down_or_down_plus_one(x: u128, y: u128, d in 1u128..u128::MAX) {
            if let (Some(dn), Some(up)) = (mul_div_down(x, y, d), mul_div_up(x, y, d)) {
                prop_assert!(up == dn || up == dn + 1);
            }
        }
    }
}
