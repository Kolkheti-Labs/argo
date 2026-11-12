//! Permissionless market creation (F3), with the dead deposit (D11).

use crate::{check_price_account, now_ms, read_config, same, token, write, Output};
use argo_core::shares::to_shares_down;
use argo_lending_core::{
    collateral_vault_id, collateral_vault_seed, dead_owner, loan_vault_id, loan_vault_seed,
    market_account_id, market_id, market_seed, oracle_digest, position_id, position_seed, Market,
    MarketParams, OracleDescriptor, Position, DEAD_DEPOSIT, MAX_MAX_AGE_MS, MIN_MAX_AGE_MS,
    ORACLE_DESCRIPTOR_VERSION,
};
use lee_core::account::{Account, AccountWithMetadata};
use lee_core::program::{AccountPostState, Claim, ProgramId};

/// Create the market `params`, bound to the feed `oracle` describes.
///
/// Claims Market, the dead-deposit Position and (through the token program)
/// both vaults, then moves `DEAD_DEPOSIT` loan units from the creator into the
/// loan vault as supply nobody can withdraw.
#[allow(
    clippy::too_many_arguments,
    reason = "one argument per account the instruction touches"
)]
pub fn create_market(
    config: AccountWithMetadata,
    market: AccountWithMetadata,
    loan_vault: AccountWithMetadata,
    collateral_vault: AccountWithMetadata,
    dead_position: AccountWithMetadata,
    loan_definition: AccountWithMetadata,
    collateral_definition: AccountWithMetadata,
    price_account: AccountWithMetadata,
    clock: AccountWithMetadata,
    creator_loan_holding: AccountWithMetadata,
    params: MarketParams,
    oracle: OracleDescriptor,
    self_program_id: ProgramId,
) -> Output {
    crate::assert_runtime_checked(&[
        &config,
        &market,
        &loan_vault,
        &collateral_vault,
        &dead_position,
        &loan_definition,
        &collateral_definition,
        &price_account,
        &clock,
        &creator_loan_holding,
    ]);
    let cfg = read_config(&config, &self_program_id);
    assert!(cfg.enabled_lltv.contains(&params.lltv), "lltv not enabled");
    assert!(cfg.enabled_irm.contains(&params.irm), "irm not enabled");
    assert_ne!(
        params.loan_token, params.collateral_token,
        "loan and collateral token must differ"
    );

    assert_eq!(
        oracle.version, ORACLE_DESCRIPTOR_VERSION,
        "unknown oracle descriptor version"
    );
    assert!(
        (MIN_MAX_AGE_MS..=MAX_MAX_AGE_MS).contains(&oracle.max_age_ms),
        "max_age_ms out of bounds"
    );
    assert_ne!(
        oracle.oracle_program,
        lee_core::program::DEFAULT_PROGRAM_ID,
        "oracle program must not be the default program id"
    );
    assert_eq!(
        params.oracle,
        oracle_digest(&oracle),
        "oracle param is not the descriptor's digest"
    );
    // The feed must exist and quote this pair (D7).
    check_price_account(&price_account, &oracle, &params);

    assert_eq!(
        loan_definition.account_id, params.loan_token,
        "wrong loan definition"
    );
    assert_eq!(
        collateral_definition.account_id, params.collateral_token,
        "wrong collateral definition"
    );
    token::assert_fungible_definition(&loan_definition);
    token::assert_fungible_definition(&collateral_definition);
    token::assert_holding_of(&creator_loan_holding, &params.loan_token);

    let id = market_id(&params);
    let dead = dead_owner(&id);
    assert_eq!(
        market.account_id,
        market_account_id(&self_program_id, &id),
        "market id mismatch"
    );
    assert_eq!(
        loan_vault.account_id,
        loan_vault_id(&self_program_id, &id),
        "loan vault id mismatch"
    );
    assert_eq!(
        collateral_vault.account_id,
        collateral_vault_id(&self_program_id, &id),
        "collateral vault id mismatch"
    );
    assert_eq!(
        dead_position.account_id,
        position_id(&self_program_id, &id, &dead),
        "dead position id mismatch"
    );
    for acc in [&market, &loan_vault, &collateral_vault, &dead_position] {
        assert_eq!(acc.account, Account::default(), "market already exists");
    }

    let now = now_ms(&clock);
    let dead_shares = to_shares_down(DEAD_DEPOSIT, 0, 0).expect("dead deposit shares fit");
    let state = Market {
        params,
        oracle,
        total_supply_assets: DEAD_DEPOSIT,
        total_supply_shares: dead_shares,
        total_borrow_assets: 0,
        total_borrow_shares: 0,
        last_update_ms: now,
        fee: 0,
        pending_fee_shares: 0,
        rate_at_target: 0,
    };
    let dead_state = Position {
        supply_shares: dead_shares,
        ..Position::default()
    };

    let posts = vec![
        same(&config),
        AccountPostState::new_claimed(write(market.account, &state), Claim::Pda(market_seed(&id))),
        same(&loan_vault),
        same(&collateral_vault),
        AccountPostState::new_claimed(
            write(dead_position.account, &dead_state),
            Claim::Pda(position_seed(&id, &dead)),
        ),
        same(&loan_definition),
        same(&collateral_definition),
        same(&price_account),
        same(&clock),
        same(&creator_loan_holding),
    ];

    let loan_vault_after_init = token::initialized_vault(loan_vault.account_id, params.loan_token);
    let calls = vec![
        token::initialize_vault(loan_definition, loan_vault, loan_vault_seed(&id)),
        token::initialize_vault(
            collateral_definition,
            collateral_vault,
            collateral_vault_seed(&id),
        ),
        token::transfer_from_user(creator_loan_holding, loan_vault_after_init, DEAD_DEPOSIT),
    ];
    (posts, calls)
}
