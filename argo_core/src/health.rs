//! Health check (Morpho Blue `_isHealthy`), with a Q64.64 oracle price
//! (`docs/m1/decisions.md` D3). Every rounding goes against the borrower:
//! collateral value and borrowing power round down, debt rounds up.

use crate::math::{mul_div_down, w_mul_down};
use crate::shares::to_assets_up;
use crate::PRICE_ONE;

/// Value of `collateral` raw collateral units in raw loan units at a Q64.64
/// `price`, rounding down. `None` on overflow.
pub fn collateral_value(collateral: u128, price: u128) -> Option<u128> {
    mul_div_down(collateral, price, PRICE_ONE)
}

/// Most a position may owe: `collateral_value × lltv / WAD`, rounding down.
pub fn max_borrow(collateral: u128, price: u128, lltv: u128) -> Option<u128> {
    w_mul_down(collateral_value(collateral, price)?, lltv)
}

/// Debt of `borrow_shares` in raw loan units, rounding up.
pub fn debt(
    borrow_shares: u128,
    total_borrow_assets: u128,
    total_borrow_shares: u128,
) -> Option<u128> {
    to_assets_up(borrow_shares, total_borrow_assets, total_borrow_shares)
}

/// `true` when the position's debt is within its borrowing power. A position
/// with no borrow shares is healthy without reading the price. `None` on
/// overflow, which callers treat as a failed check.
pub fn is_healthy(
    borrow_shares: u128,
    total_borrow_assets: u128,
    total_borrow_shares: u128,
    collateral: u128,
    price: u128,
    lltv: u128,
) -> Option<bool> {
    if borrow_shares == 0 {
        return Some(true);
    }
    let owed = debt(borrow_shares, total_borrow_assets, total_borrow_shares)?;
    Some(max_borrow(collateral, price, lltv)? >= owed)
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    reason = "test inputs are bounded far below u128::MAX"
)]
mod tests {
    use super::*;
    use crate::WAD;

    const LLTV_86: u128 = 860_000_000_000_000_000;

    #[test]
    fn price_one_is_identity() {
        assert_eq!(collateral_value(1_000, PRICE_ONE), Some(1_000));
    }

    #[test]
    fn no_debt_is_healthy_at_zero_price() {
        assert_eq!(is_healthy(0, 0, 0, 0, 0, LLTV_86), Some(true));
    }

    #[test]
    fn boundary_is_healthy_one_more_is_not() {
        // 1000 collateral at price 1, LLTV 86% => borrowing power 860.
        // Empty market: 860 assets of debt = 860e6 shares (virtual shares 1e6).
        let shares_860 = 860_000_000;
        assert_eq!(
            is_healthy(shares_860, 860, shares_860, 1_000, PRICE_ONE, LLTV_86),
            Some(true)
        );
        let shares_861 = 861_000_000;
        assert_eq!(
            is_healthy(shares_861, 861, shares_861, 1_000, PRICE_ONE, LLTV_86),
            Some(false)
        );
    }

    #[test]
    fn wide_decimal_gap_keeps_precision() {
        // Cheap 18-decimal collateral priced in a 6-decimal loan token:
        // one whole collateral (1e18 raw) worth 0.5 whole loan (5e5 raw) gives a
        // raw/raw price of 5e5 / 1e18 = 5e-13, i.e. Q64.64 ~= 9_223_372.
        let price = (PRICE_ONE / 2) / 1_000_000_000_000;
        let value = collateral_value(1_000_000_000_000_000_000, price).expect("fits");
        // Within 1e-6 relative of the exact 500_000.
        assert!((499_999..=500_000).contains(&value), "value = {value}");
        assert_eq!(max_borrow(0, price, WAD), Some(0));
    }

    #[test]
    fn non_exact_ratios_round_against_the_borrower() {
        // 1.5 loan units of value floors to 1; 0.5 + 2^-64 floors to 0.
        assert_eq!(collateral_value(3, PRICE_ONE / 2), Some(1));
        assert_eq!(collateral_value(1, PRICE_ONE / 2 + 1), Some(0));
        // Borrowing power floors: 1 unit at price 1 and LLTV 0.86 is 0.
        assert_eq!(max_borrow(1, PRICE_ONE, LLTV_86), Some(0));
        // Debt ceils: 1 share of (assets 1, shares 3) is 2 / 1_000_003 assets -> 1.
        assert_eq!(debt(1, 1, 3), Some(1));
        // So a 1-share debt against 1 unit of collateral is unhealthy.
        assert_eq!(is_healthy(1, 1, 3, 1, PRICE_ONE, LLTV_86), Some(false));
    }

    #[test]
    fn overflow_is_none_not_wrap() {
        assert_eq!(collateral_value(u128::MAX, u128::MAX), None);
    }
}
