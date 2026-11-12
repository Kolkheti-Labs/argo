//! The six core operations (F4–F7), Morpho Blue semantics: accrue first,
//! round against the user, check health after borrowing or removing collateral,
//! keep `total_borrow_assets <= total_supply_assets`.
//!
//! In M1 `on_behalf` must be the authorised `caller` for withdraw, borrow and
//! withdraw_collateral; manager authorisation (F17) is M3.

use crate::{
    accrue, assert_exactly_one, now_ms, read_market, read_position, read_price, same, token, write,
    Output,
};
use argo_core::health::is_healthy;
use argo_core::shares::{to_assets_down, to_assets_up, to_shares_down, to_shares_up};
use argo_lending_core::{
    collateral_vault_id, collateral_vault_seed, loan_vault_id, loan_vault_seed, position_seed,
    Market,
};
use lee_core::account::{AccountId, AccountWithMetadata};
use lee_core::program::{AccountPostState, ProgramId};

/// The position owner acting in person. It must be program-owned (for example a
/// token holding): an unowned account is dropped from Argo's output once it has
/// signed anything, so it could act once and never again (D15).
fn assert_caller(caller: &AccountWithMetadata, on_behalf: &AccountId) {
    assert!(caller.is_authorized, "caller must be authorised");
    assert_ne!(
        caller.account.program_owner,
        lee_core::program::DEFAULT_PROGRAM_ID,
        "caller must be a program-owned account"
    );
    assert_eq!(&caller.account_id, on_behalf, "caller is not on_behalf");
}

fn assert_loan_vault(vault: &AccountWithMetadata, m: &Market, id: &[u8; 32], pid: &ProgramId) {
    assert_eq!(
        vault.account_id,
        loan_vault_id(pid, id),
        "loan vault id mismatch"
    );
    token::assert_holding_of(vault, &m.params.loan_token);
}

fn assert_collateral_vault(
    vault: &AccountWithMetadata,
    m: &Market,
    id: &[u8; 32],
    pid: &ProgramId,
) {
    assert_eq!(
        vault.account_id,
        collateral_vault_id(pid, id),
        "collateral vault id mismatch"
    );
    token::assert_holding_of(vault, &m.params.collateral_token);
}

/// A user's payer or receiver holding: a holding of `definition` owned by the
/// pinned token program, and not one of the market's vaults (spec §3, "Credited
/// holdings must exist" and aliasing).
fn assert_user_holding(
    holding: &AccountWithMetadata,
    definition: &AccountId,
    id: &[u8; 32],
    pid: &ProgramId,
) {
    assert_ne!(
        holding.account_id,
        loan_vault_id(pid, id),
        "holding is the loan vault"
    );
    assert_ne!(
        holding.account_id,
        collateral_vault_id(pid, id),
        "holding is the collateral vault"
    );
    token::assert_holding_of(holding, definition);
}

fn assert_liquidity(m: &Market) {
    assert!(
        m.total_borrow_assets <= m.total_supply_assets,
        "insufficient liquidity"
    );
}

fn assert_healthy(m: &Market, p: &argo_lending_core::Position, price: u128) {
    let ok = is_healthy(
        p.borrow_shares,
        m.total_borrow_assets,
        m.total_borrow_shares,
        p.collateral,
        price,
        m.params.lltv,
    )
    .expect("health check overflow");
    assert!(ok, "position would be unhealthy");
}

/// Supply loan assets on behalf of `on_behalf`.
#[allow(
    clippy::too_many_arguments,
    reason = "one argument per account the instruction touches"
)]
pub fn supply(
    market: AccountWithMetadata,
    position: AccountWithMetadata,
    loan_vault: AccountWithMetadata,
    payer_holding: AccountWithMetadata,
    clock: AccountWithMetadata,
    assets: u128,
    shares: u128,
    on_behalf: AccountId,
    self_program_id: ProgramId,
) -> Output {
    crate::assert_runtime_checked(&[&market, &position, &loan_vault, &payer_holding, &clock]);
    assert_exactly_one(assets, shares);
    let (mut m, id) = read_market(&market, &self_program_id);
    assert_loan_vault(&loan_vault, &m, &id, &self_program_id);
    assert_user_holding(&payer_holding, &m.params.loan_token, &id, &self_program_id);
    let (mut p, claim) = read_position(&position, &self_program_id, &id, &on_behalf);
    accrue(&mut m, now_ms(&clock));

    let (assets, shares) = if assets > 0 {
        (
            assets,
            to_shares_down(assets, m.total_supply_assets, m.total_supply_shares)
                .expect("shares overflow"),
        )
    } else {
        (
            to_assets_up(shares, m.total_supply_assets, m.total_supply_shares)
                .expect("assets overflow"),
            shares,
        )
    };
    assert_ne!(shares, 0, "zero shares");
    p.supply_shares = p
        .supply_shares
        .checked_add(shares)
        .expect("position overflow");
    m.total_supply_shares = m
        .total_supply_shares
        .checked_add(shares)
        .expect("supply shares overflow");
    m.total_supply_assets = m
        .total_supply_assets
        .checked_add(assets)
        .expect("supply assets overflow");

    let posts = vec![
        AccountPostState::new(write(market.account, &m)),
        crate::position_post(position, &p, claim, position_seed(&id, &on_behalf)),
        same(&loan_vault),
        same(&payer_holding),
        same(&clock),
    ];
    (
        posts,
        vec![token::transfer_from_user(payer_holding, loan_vault, assets)],
    )
}

/// Withdraw supplied loan assets to `receiver_holding`.
#[allow(
    clippy::too_many_arguments,
    reason = "one argument per account the instruction touches"
)]
pub fn withdraw(
    market: AccountWithMetadata,
    position: AccountWithMetadata,
    loan_vault: AccountWithMetadata,
    receiver_holding: AccountWithMetadata,
    caller: AccountWithMetadata,
    clock: AccountWithMetadata,
    assets: u128,
    shares: u128,
    on_behalf: AccountId,
    self_program_id: ProgramId,
) -> Output {
    crate::assert_runtime_checked(&[
        &market,
        &position,
        &loan_vault,
        &receiver_holding,
        &caller,
        &clock,
    ]);
    assert_exactly_one(assets, shares);
    assert_caller(&caller, &on_behalf);
    let (mut m, id) = read_market(&market, &self_program_id);
    assert_loan_vault(&loan_vault, &m, &id, &self_program_id);
    assert_user_holding(
        &receiver_holding,
        &m.params.loan_token,
        &id,
        &self_program_id,
    );
    let (mut p, claim) = read_position(&position, &self_program_id, &id, &on_behalf);
    assert!(!claim, "no position");
    accrue(&mut m, now_ms(&clock));

    let (assets, shares) = if assets > 0 {
        (
            assets,
            to_shares_up(assets, m.total_supply_assets, m.total_supply_shares)
                .expect("shares overflow"),
        )
    } else {
        (
            to_assets_down(shares, m.total_supply_assets, m.total_supply_shares)
                .expect("assets overflow"),
            shares,
        )
    };
    p.supply_shares = p
        .supply_shares
        .checked_sub(shares)
        .expect("insufficient supply shares");
    m.total_supply_shares = m
        .total_supply_shares
        .checked_sub(shares)
        .expect("supply shares underflow");
    m.total_supply_assets = m
        .total_supply_assets
        .checked_sub(assets)
        .expect("supply assets underflow");
    assert_liquidity(&m);

    let posts = vec![
        AccountPostState::new(write(market.account, &m)),
        AccountPostState::new(write(position.account, &p)),
        same(&loan_vault),
        same(&receiver_holding),
        same(&caller),
        same(&clock),
    ];
    let seed = loan_vault_seed(&id);
    (
        posts,
        vec![token::transfer_from_vault(
            loan_vault,
            receiver_holding,
            assets,
            seed,
        )],
    )
}

/// Add collateral on behalf of `on_behalf`. Does not accrue (Morpho).
pub fn supply_collateral(
    market: AccountWithMetadata,
    position: AccountWithMetadata,
    collateral_vault: AccountWithMetadata,
    payer_holding: AccountWithMetadata,
    assets: u128,
    on_behalf: AccountId,
    self_program_id: ProgramId,
) -> Output {
    crate::assert_runtime_checked(&[&market, &position, &collateral_vault, &payer_holding]);
    assert_ne!(assets, 0, "zero assets");
    let (m, id) = read_market(&market, &self_program_id);
    assert_collateral_vault(&collateral_vault, &m, &id, &self_program_id);
    assert_user_holding(
        &payer_holding,
        &m.params.collateral_token,
        &id,
        &self_program_id,
    );
    let (mut p, claim) = read_position(&position, &self_program_id, &id, &on_behalf);
    p.collateral = p
        .collateral
        .checked_add(assets)
        .expect("collateral overflow");

    let posts = vec![
        same(&market),
        crate::position_post(position, &p, claim, position_seed(&id, &on_behalf)),
        same(&collateral_vault),
        same(&payer_holding),
    ];
    (
        posts,
        vec![token::transfer_from_user(
            payer_holding,
            collateral_vault,
            assets,
        )],
    )
}

/// Remove collateral to `receiver_holding`; the position must stay healthy.
/// The price is read only when the position has debt (Morpho `withdrawCollateral`).
#[allow(
    clippy::too_many_arguments,
    reason = "one argument per account the instruction touches"
)]
pub fn withdraw_collateral(
    market: AccountWithMetadata,
    position: AccountWithMetadata,
    collateral_vault: AccountWithMetadata,
    receiver_holding: AccountWithMetadata,
    caller: AccountWithMetadata,
    price_account: AccountWithMetadata,
    clock: AccountWithMetadata,
    assets: u128,
    on_behalf: AccountId,
    self_program_id: ProgramId,
) -> Output {
    crate::assert_runtime_checked(&[
        &market,
        &position,
        &collateral_vault,
        &receiver_holding,
        &caller,
        &price_account,
        &clock,
    ]);
    assert_ne!(assets, 0, "zero assets");
    assert_caller(&caller, &on_behalf);
    let (mut m, id) = read_market(&market, &self_program_id);
    assert_collateral_vault(&collateral_vault, &m, &id, &self_program_id);
    assert_user_holding(
        &receiver_holding,
        &m.params.collateral_token,
        &id,
        &self_program_id,
    );
    let (mut p, claim) = read_position(&position, &self_program_id, &id, &on_behalf);
    assert!(!claim, "no position");
    let now = now_ms(&clock);
    accrue(&mut m, now);
    p.collateral = p
        .collateral
        .checked_sub(assets)
        .expect("insufficient collateral");
    if p.borrow_shares > 0 {
        let price = read_price(&price_account, &m.oracle, &m.params, now);
        assert_healthy(&m, &p, price);
    }

    let posts = vec![
        AccountPostState::new(write(market.account, &m)),
        AccountPostState::new(write(position.account, &p)),
        same(&collateral_vault),
        same(&receiver_holding),
        same(&caller),
        same(&price_account),
        same(&clock),
    ];
    let seed = collateral_vault_seed(&id);
    (
        posts,
        vec![token::transfer_from_vault(
            collateral_vault,
            receiver_holding,
            assets,
            seed,
        )],
    )
}

/// Borrow loan assets to `receiver_holding` against the position's collateral.
#[allow(
    clippy::too_many_arguments,
    reason = "one argument per account the instruction touches"
)]
pub fn borrow(
    market: AccountWithMetadata,
    position: AccountWithMetadata,
    loan_vault: AccountWithMetadata,
    receiver_holding: AccountWithMetadata,
    caller: AccountWithMetadata,
    price_account: AccountWithMetadata,
    clock: AccountWithMetadata,
    assets: u128,
    shares: u128,
    on_behalf: AccountId,
    self_program_id: ProgramId,
) -> Output {
    crate::assert_runtime_checked(&[
        &market,
        &position,
        &loan_vault,
        &receiver_holding,
        &caller,
        &price_account,
        &clock,
    ]);
    assert_exactly_one(assets, shares);
    assert_caller(&caller, &on_behalf);
    let (mut m, id) = read_market(&market, &self_program_id);
    assert_loan_vault(&loan_vault, &m, &id, &self_program_id);
    assert_user_holding(
        &receiver_holding,
        &m.params.loan_token,
        &id,
        &self_program_id,
    );
    let (mut p, claim) = read_position(&position, &self_program_id, &id, &on_behalf);
    assert!(!claim, "no position");
    let now = now_ms(&clock);
    accrue(&mut m, now);

    let (assets, shares) = if assets > 0 {
        (
            assets,
            to_shares_up(assets, m.total_borrow_assets, m.total_borrow_shares)
                .expect("shares overflow"),
        )
    } else {
        (
            to_assets_down(shares, m.total_borrow_assets, m.total_borrow_shares)
                .expect("assets overflow"),
            shares,
        )
    };
    p.borrow_shares = p
        .borrow_shares
        .checked_add(shares)
        .expect("position overflow");
    m.total_borrow_shares = m
        .total_borrow_shares
        .checked_add(shares)
        .expect("borrow shares overflow");
    m.total_borrow_assets = m
        .total_borrow_assets
        .checked_add(assets)
        .expect("borrow assets overflow");
    let price = read_price(&price_account, &m.oracle, &m.params, now);
    assert_healthy(&m, &p, price);
    assert_liquidity(&m);

    let posts = vec![
        AccountPostState::new(write(market.account, &m)),
        AccountPostState::new(write(position.account, &p)),
        same(&loan_vault),
        same(&receiver_holding),
        same(&caller),
        same(&price_account),
        same(&clock),
    ];
    let seed = loan_vault_seed(&id);
    (
        posts,
        vec![token::transfer_from_vault(
            loan_vault,
            receiver_holding,
            assets,
            seed,
        )],
    )
}

/// Repay debt on behalf of `on_behalf`.
#[allow(
    clippy::too_many_arguments,
    reason = "one argument per account the instruction touches"
)]
pub fn repay(
    market: AccountWithMetadata,
    position: AccountWithMetadata,
    loan_vault: AccountWithMetadata,
    payer_holding: AccountWithMetadata,
    clock: AccountWithMetadata,
    assets: u128,
    shares: u128,
    on_behalf: AccountId,
    self_program_id: ProgramId,
) -> Output {
    crate::assert_runtime_checked(&[&market, &position, &loan_vault, &payer_holding, &clock]);
    assert_exactly_one(assets, shares);
    let (mut m, id) = read_market(&market, &self_program_id);
    assert_loan_vault(&loan_vault, &m, &id, &self_program_id);
    assert_user_holding(&payer_holding, &m.params.loan_token, &id, &self_program_id);
    let (mut p, claim) = read_position(&position, &self_program_id, &id, &on_behalf);
    assert!(!claim, "no position");
    accrue(&mut m, now_ms(&clock));

    let (assets, shares) = if assets > 0 {
        (
            assets,
            to_shares_down(assets, m.total_borrow_assets, m.total_borrow_shares)
                .expect("shares overflow"),
        )
    } else {
        (
            to_assets_up(shares, m.total_borrow_assets, m.total_borrow_shares)
                .expect("assets overflow"),
            shares,
        )
    };
    assert_ne!(shares, 0, "zero shares");
    p.borrow_shares = p
        .borrow_shares
        .checked_sub(shares)
        .expect("repaying more than owed");
    m.total_borrow_shares = m
        .total_borrow_shares
        .checked_sub(shares)
        .expect("borrow shares underflow");
    // Morpho `zeroFloorSub`: rounding can make `assets` exceed the total by one.
    m.total_borrow_assets = m.total_borrow_assets.saturating_sub(assets);

    let posts = vec![
        AccountPostState::new(write(market.account, &m)),
        AccountPostState::new(write(position.account, &p)),
        same(&loan_vault),
        same(&payer_holding),
        same(&clock),
    ];
    (
        posts,
        vec![token::transfer_from_user(payer_holding, loan_vault, assets)],
    )
}
