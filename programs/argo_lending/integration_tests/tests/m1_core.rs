//! M1 core market primitives against the in-process LEZ v0.2.4 state machine,
//! with the pinned lez-programs v2.0.1 token program (vendored binary) and the
//! Argo guest. The price account is inserted directly with a mock owner: M1
//! tests the read path; real feeds are M4.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "tests"
)]

use argo_core::PRICE_ONE;
use argo_lending_core::external::{
    ClockAccountData, OraclePriceAccount, TokenDefinition, TokenHolding, CLOCK_01_ACCOUNT_ID,
    TOKEN_PROGRAM_ID,
};
use argo_lending_core::{
    collateral_vault_id, config_id, dead_owner, loan_vault_id, market_account_id, market_id,
    oracle_digest, position_id, Instruction, Market, MarketParams, OracleDescriptor, Position,
    DEAD_DEPOSIT, ORACLE_DESCRIPTOR_VERSION,
};
use lee::{
    program_deployment_transaction, public_transaction, PrivateKey, ProgramDeploymentTransaction,
    PublicKey, PublicTransaction, V03State,
};
use lee_core::account::{Account, AccountId, Data, Nonce};
use lee_core::program::ProgramId;

const TOKEN_BIN: &[u8] = include_bytes!("../../../../vendor/lez-programs-v2.0.1/token.bin");
const TWAP_BIN: &[u8] = include_bytes!("../../../../vendor/lez-programs-v2.0.1/twap_oracle.bin");
/// twap_oracle on testnet-legacy: `5681e27c…` as little-endian words.
const TWAP_PROGRAM_ID: ProgramId = [
    2_095_219_030,
    3_840_717_607,
    3_143_745_959,
    1_848_566_264,
    2_577_820_878,
    2_004_592_677,
    4_203_995_641,
    832_250_954,
];
const LLTV: u128 = 860_000_000_000_000_000;
const IRM: ProgramId = [7; 8];
const MOCK_ORACLE: ProgramId = [9; 8];
const MAX_AGE_MS: u64 = 60_000;
const T0: u64 = 1_700_000_000_000;
const START: u128 = 10_000_000;
/// Stand-in owner for CLOCK_01. On chain the clock program owns it from genesis;
/// it must be non-default, or SPEL drops the read-only input from the output.
const CLOCK_OWNER: ProgramId = [3; 8];

fn argo() -> ProgramId {
    argo_lending_methods::ARGO_LENDING_ID
}
fn key(b: u8) -> PrivateKey {
    PrivateKey::try_new([b; 32]).expect("valid key")
}
fn id_of(k: &PrivateKey) -> AccountId {
    AccountId::from(&PublicKey::new_from_private_key(k))
}

// Actors. One account holds one token, so a user with two tokens has two keys.
fn admin() -> PrivateKey {
    key(1)
}
/// Lender and market creator: loan-token holding.
fn alice() -> PrivateKey {
    key(2)
}
/// Borrower identity and collateral holding.
fn bob() -> PrivateKey {
    key(3)
}
/// Bob's loan-token holding (receives the borrow, pays the repay).
fn bob_loan() -> PrivateKey {
    key(4)
}
/// Bob's second collateral holding (receives withdrawn collateral).
fn bob_coll2() -> PrivateKey {
    key(5)
}
/// Alice's second loan holding (receives withdrawn supply).
fn alice2() -> PrivateKey {
    key(6)
}

fn loan_def() -> AccountId {
    AccountId::new([11; 32])
}
fn coll_def() -> AccountId {
    AccountId::new([12; 32])
}
fn price_acc() -> AccountId {
    AccountId::new([13; 32])
}

fn definition(name: &str) -> Account {
    Account {
        program_owner: TOKEN_PROGRAM_ID,
        balance: 0,
        data: Data::try_from(
            borsh::to_vec(&TokenDefinition::Fungible {
                name: name.to_owned(),
                total_supply: START * 10,
                metadata_id: None,
                authority: None,
            })
            .unwrap(),
        )
        .unwrap(),
        nonce: Nonce(0),
    }
}

fn holding(def: AccountId, balance: u128) -> Account {
    Account {
        program_owner: TOKEN_PROGRAM_ID,
        balance: 0,
        data: Data::try_from(
            borsh::to_vec(&TokenHolding::Fungible {
                definition_id: def,
                balance,
            })
            .unwrap(),
        )
        .unwrap(),
        nonce: Nonce(0),
    }
}

fn set_clock(st: &mut V03State, ts: u64) {
    st.force_insert_account(
        CLOCK_01_ACCOUNT_ID,
        Account {
            data: Data::try_from(
                borsh::to_vec(&ClockAccountData {
                    block_id: 1,
                    timestamp: ts,
                })
                .unwrap(),
            )
            .unwrap(),
            program_owner: CLOCK_OWNER,
            ..Account::default()
        },
    );
}

fn set_price(
    st: &mut V03State,
    id: AccountId,
    base: AccountId,
    quote: AccountId,
    price: u128,
    ts: u64,
) {
    st.force_insert_account(
        id,
        Account {
            program_owner: MOCK_ORACLE,
            data: Data::try_from(
                borsh::to_vec(&OraclePriceAccount {
                    base_asset: base,
                    quote_asset: quote,
                    price,
                    timestamp: ts,
                    source_id: AccountId::new([0; 32]),
                    confidence_interval: 0,
                })
                .unwrap(),
            )
            .unwrap(),
            ..Account::default()
        },
    );
}

fn deploy(st: &mut V03State, bytes: &[u8]) {
    st.transition_from_program_deployment_transaction(&ProgramDeploymentTransaction::new(
        program_deployment_transaction::Message::new(bytes.to_vec()),
    ))
    .expect("deploy");
}

fn submit(
    st: &mut V03State,
    accounts: Vec<AccountId>,
    signers: &[&PrivateKey],
    ix: &Instruction,
) -> Result<(), lee::error::LeeError> {
    let nonces = signers
        .iter()
        .map(|k| st.get_account_by_id(id_of(k)).nonce)
        .collect();
    let msg = public_transaction::Message::try_new(argo(), accounts, nonces, ix)?;
    let ws = public_transaction::WitnessSet::for_message(&msg, signers);
    st.transition_from_public_transaction(&PublicTransaction::new(msg, ws), 1, T0)
}

fn balance(st: &V03State, id: AccountId) -> u128 {
    let h: TokenHolding = borsh::from_slice(st.get_account_by_id(id).data.as_ref()).unwrap();
    match h {
        TokenHolding::Fungible { balance, .. } => balance,
        TokenHolding::NftMaster { .. } | TokenHolding::NftPrintedCopy { .. } => {
            panic!("not fungible")
        }
    }
}

fn descriptor() -> OracleDescriptor {
    OracleDescriptor {
        version: ORACLE_DESCRIPTOR_VERSION,
        oracle_program: MOCK_ORACLE,
        price_account: price_acc(),
        max_age_ms: MAX_AGE_MS,
    }
}

fn params() -> MarketParams {
    MarketParams {
        loan_token: loan_def(),
        collateral_token: coll_def(),
        oracle: oracle_digest(&descriptor()),
        irm: IRM,
        lltv: LLTV,
    }
}

struct Ids {
    mid: [u8; 32],
    market: AccountId,
    loan_vault: AccountId,
    coll_vault: AccountId,
}

fn ids() -> Ids {
    let mid = market_id(&params());
    Ids {
        mid,
        market: market_account_id(&argo(), &mid),
        loan_vault: loan_vault_id(&argo(), &mid),
        coll_vault: collateral_vault_id(&argo(), &mid),
    }
}

fn position(owner: &AccountId) -> AccountId {
    position_id(&argo(), &ids().mid, owner)
}

fn market_state(st: &V03State) -> Market {
    borsh::from_slice(st.get_account_by_id(ids().market).data.as_ref()).unwrap()
}

fn position_state(st: &V03State, owner: &AccountId) -> Position {
    borsh::from_slice(st.get_account_by_id(position(owner)).data.as_ref()).unwrap()
}

/// Deployed programs, tokens, clock and price; Config initialised with one LLTV
/// and one IRM enabled. No market yet.
fn base_state() -> V03State {
    let mut st = V03State::new();
    deploy(&mut st, TOKEN_BIN);
    deploy(&mut st, argo_lending_methods::ARGO_LENDING_ELF);
    st.force_insert_account(loan_def(), definition("LOAN"));
    st.force_insert_account(coll_def(), definition("COLL"));
    st.force_insert_account(id_of(&alice()), holding(loan_def(), START));
    st.force_insert_account(id_of(&alice2()), holding(loan_def(), 0));
    st.force_insert_account(id_of(&bob()), holding(coll_def(), START));
    st.force_insert_account(id_of(&bob_loan()), holding(loan_def(), 0));
    st.force_insert_account(id_of(&bob_coll2()), holding(coll_def(), 0));
    set_clock(&mut st, T0);
    set_price(&mut st, price_acc(), coll_def(), loan_def(), PRICE_ONE, T0);

    let cfg = config_id(&argo());
    let a = id_of(&admin());
    submit(&mut st, vec![cfg, a], &[&admin()], &Instruction::Initialize).expect("initialize");
    submit(
        &mut st,
        vec![cfg, a],
        &[&admin()],
        &Instruction::EnableLltv { lltv: LLTV },
    )
    .expect("enable lltv");
    submit(
        &mut st,
        vec![cfg, a],
        &[&admin()],
        &Instruction::EnableIrm { irm: IRM },
    )
    .expect("enable irm");
    st
}

fn create_market_accounts(p: &MarketParams, d: &OracleDescriptor) -> Vec<AccountId> {
    let mid = market_id(p);
    vec![
        config_id(&argo()),
        market_account_id(&argo(), &mid),
        loan_vault_id(&argo(), &mid),
        collateral_vault_id(&argo(), &mid),
        position_id(&argo(), &mid, &dead_owner(&mid)),
        p.loan_token,
        p.collateral_token,
        d.price_account,
        CLOCK_01_ACCOUNT_ID,
        id_of(&alice()),
    ]
}

fn create(
    st: &mut V03State,
    p: MarketParams,
    d: OracleDescriptor,
) -> Result<(), lee::error::LeeError> {
    submit(
        st,
        create_market_accounts(&p, &d),
        &[&alice()],
        &Instruction::CreateMarket {
            params: p,
            oracle: d,
        },
    )
}

fn market_state_with_market() -> V03State {
    let mut st = base_state();
    create(&mut st, params(), descriptor()).expect("create market");
    st
}

fn supply(
    st: &mut V03State,
    payer: &PrivateKey,
    assets: u128,
    on_behalf: AccountId,
) -> Result<(), lee::error::LeeError> {
    let i = ids();
    submit(
        st,
        vec![
            i.market,
            position(&on_behalf),
            i.loan_vault,
            id_of(payer),
            CLOCK_01_ACCOUNT_ID,
        ],
        &[payer],
        &Instruction::Supply {
            assets,
            shares: 0,
            on_behalf,
        },
    )
}

fn supply_collateral(
    st: &mut V03State,
    payer: &PrivateKey,
    assets: u128,
    on_behalf: AccountId,
) -> Result<(), lee::error::LeeError> {
    let i = ids();
    submit(
        st,
        vec![i.market, position(&on_behalf), i.coll_vault, id_of(payer)],
        &[payer],
        &Instruction::SupplyCollateral { assets, on_behalf },
    )
}

fn borrow(
    st: &mut V03State,
    caller: &PrivateKey,
    receiver: AccountId,
    assets: u128,
    price_account: AccountId,
) -> Result<(), lee::error::LeeError> {
    let i = ids();
    let on_behalf = id_of(caller);
    submit(
        st,
        vec![
            i.market,
            position(&on_behalf),
            i.loan_vault,
            receiver,
            on_behalf,
            price_account,
            CLOCK_01_ACCOUNT_ID,
        ],
        &[caller],
        &Instruction::Borrow {
            assets,
            shares: 0,
            on_behalf,
        },
    )
}

fn repay(
    st: &mut V03State,
    payer: &PrivateKey,
    assets: u128,
    on_behalf: AccountId,
) -> Result<(), lee::error::LeeError> {
    let i = ids();
    submit(
        st,
        vec![
            i.market,
            position(&on_behalf),
            i.loan_vault,
            id_of(payer),
            CLOCK_01_ACCOUNT_ID,
        ],
        &[payer],
        &Instruction::Repay {
            assets,
            shares: 0,
            on_behalf,
        },
    )
}

fn withdraw_collateral(
    st: &mut V03State,
    caller: &PrivateKey,
    receiver: AccountId,
    assets: u128,
) -> Result<(), lee::error::LeeError> {
    let i = ids();
    let on_behalf = id_of(caller);
    submit(
        st,
        vec![
            i.market,
            position(&on_behalf),
            i.coll_vault,
            receiver,
            on_behalf,
            price_acc(),
            CLOCK_01_ACCOUNT_ID,
        ],
        &[caller],
        &Instruction::WithdrawCollateral { assets, on_behalf },
    )
}

fn withdraw(
    st: &mut V03State,
    caller: &PrivateKey,
    on_behalf: AccountId,
    receiver: AccountId,
    assets: u128,
) -> Result<(), lee::error::LeeError> {
    let i = ids();
    submit(
        st,
        vec![
            i.market,
            position(&on_behalf),
            i.loan_vault,
            receiver,
            id_of(caller),
            CLOCK_01_ACCOUNT_ID,
        ],
        &[caller],
        &Instruction::Withdraw {
            assets,
            shares: 0,
            on_behalf,
        },
    )
}

/// Assert the transaction failed for the stated reason, not some other error.
fn fails_with(res: Result<(), lee::error::LeeError>, reason: &str) {
    let e = res.expect_err(&format!("expected failure: {reason}"));
    let msg = format!("{e:?}");
    assert!(
        msg.contains(reason),
        "failed for another reason: {msg}\n  expected: {reason}"
    );
}

fn withdraw_shares(
    st: &mut V03State,
    caller: &PrivateKey,
    receiver: AccountId,
    shares: u128,
) -> Result<(), lee::error::LeeError> {
    let i = ids();
    let on_behalf = id_of(caller);
    submit(
        st,
        vec![
            i.market,
            position(&on_behalf),
            i.loan_vault,
            receiver,
            on_behalf,
            CLOCK_01_ACCOUNT_ID,
        ],
        &[caller],
        &Instruction::Withdraw {
            assets: 0,
            shares,
            on_behalf,
        },
    )
}

// ---------------------------------------------------------------- tests ----

/// The vendored binary is the build testnet-legacy runs (D1).
#[test]
fn vendored_token_binary_has_the_pinned_program_id() {
    let program =
        lee::program::Program::new(std::borrow::Cow::Borrowed(TOKEN_BIN)).expect("valid program");
    assert_eq!(program.id(), TOKEN_PROGRAM_ID);
    let twap =
        lee::program::Program::new(std::borrow::Cow::Borrowed(TWAP_BIN)).expect("valid program");
    assert_eq!(twap.id(), TWAP_PROGRAM_ID);
}

/// F3, F4–F7, health check: all six operations, in order, with balances.
#[test]
fn six_operations_end_to_end() {
    let mut st = market_state_with_market();
    let i = ids();
    let (a, b) = (id_of(&alice()), id_of(&bob()));

    // create_market moved the dead deposit and claimed every account.
    assert_eq!(balance(&st, i.loan_vault), DEAD_DEPOSIT);
    assert_eq!(balance(&st, i.coll_vault), 0);
    assert_eq!(balance(&st, a), START - DEAD_DEPOSIT);
    let m = market_state(&st);
    assert_eq!(m.total_supply_assets, DEAD_DEPOSIT);
    assert_eq!(m.last_update_ms, T0);
    assert_eq!(
        position_state(&st, &dead_owner(&i.mid)).supply_shares,
        m.total_supply_shares
    );

    supply(&mut st, &alice(), 1_000_000, a).expect("supply");
    assert_eq!(balance(&st, i.loan_vault), DEAD_DEPOSIT + 1_000_000);

    supply_collateral(&mut st, &bob(), 2_000_000, b).expect("supply_collateral");
    assert_eq!(balance(&st, i.coll_vault), 2_000_000);
    assert_eq!(position_state(&st, &b).collateral, 2_000_000);

    borrow(&mut st, &bob(), id_of(&bob_loan()), 500_000, price_acc()).expect("borrow");
    assert_eq!(balance(&st, id_of(&bob_loan())), 500_000);
    assert_eq!(market_state(&st).total_borrow_assets, 500_000);

    repay(&mut st, &bob_loan(), 200_000, b).expect("repay");
    assert_eq!(balance(&st, id_of(&bob_loan())), 300_000);
    assert_eq!(market_state(&st).total_borrow_assets, 300_000);

    withdraw_collateral(&mut st, &bob(), id_of(&bob_coll2()), 1_000_000)
        .expect("withdraw_collateral");
    assert_eq!(balance(&st, id_of(&bob_coll2())), 1_000_000);
    assert_eq!(position_state(&st, &b).collateral, 1_000_000);

    withdraw(&mut st, &alice(), a, id_of(&alice2()), 100_000).expect("withdraw");
    assert_eq!(balance(&st, id_of(&alice2())), 100_000);

    // Vault balances equal the accounting: supplied − borrowed + repaid − withdrawn.
    let m = market_state(&st);
    assert_eq!(
        balance(&st, i.loan_vault),
        m.total_supply_assets - m.total_borrow_assets
    );
    assert_eq!(balance(&st, i.coll_vault), 1_000_000);
}

/// Health check on borrow: one unit past LLTV fails and changes nothing.
#[test]
fn borrow_past_lltv_is_rejected() {
    let mut st = market_state_with_market();
    let b = id_of(&bob());
    supply(&mut st, &alice(), 5_000_000, id_of(&alice())).unwrap();
    supply_collateral(&mut st, &bob(), 1_000_000, b).unwrap();
    let before = market_state(&st);
    // Borrowing power: 1_000_000 × 1.0 × 0.86 = 860_000.
    fails_with(
        borrow(&mut st, &bob(), id_of(&bob_loan()), 860_001, price_acc()),
        "position would be unhealthy",
    );
    assert_eq!(market_state(&st), before);
    borrow(&mut st, &bob(), id_of(&bob_loan()), 860_000, price_acc()).expect("exactly at LLTV");
}

/// Health check on withdraw_collateral when the position has debt.
#[test]
fn withdraw_collateral_that_breaks_health_is_rejected() {
    let mut st = market_state_with_market();
    let b = id_of(&bob());
    supply(&mut st, &alice(), 5_000_000, id_of(&alice())).unwrap();
    supply_collateral(&mut st, &bob(), 1_000_000, b).unwrap();
    borrow(&mut st, &bob(), id_of(&bob_loan()), 860_000, price_acc()).unwrap();
    fails_with(
        withdraw_collateral(&mut st, &bob(), id_of(&bob_coll2()), 1),
        "position would be unhealthy",
    );
}

/// D5: a price older than max_age blocks borrowing; collateral with no debt can
/// still be withdrawn because the price is not read.
#[test]
fn stale_price_blocks_borrow_but_not_debt_free_exit() {
    let mut st = market_state_with_market();
    let b = id_of(&bob());
    supply(&mut st, &alice(), 5_000_000, id_of(&alice())).unwrap();
    supply_collateral(&mut st, &bob(), 1_000_000, b).unwrap();
    set_clock(&mut st, T0 + MAX_AGE_MS + 1);
    fails_with(
        borrow(&mut st, &bob(), id_of(&bob_loan()), 1, price_acc()),
        "oracle price is stale",
    );
    withdraw_collateral(&mut st, &bob(), id_of(&bob_coll2()), 1_000_000)
        .expect("no debt, no price read");
    set_clock(&mut st, T0 + MAX_AGE_MS);
    supply_collateral(&mut st, &bob_coll2(), 1_000_000, b).unwrap();
    borrow(&mut st, &bob(), id_of(&bob_loan()), 1, price_acc()).expect("exactly max_age is fresh");
}

/// D2: a second price account with the same owner and pair is not the market's feed.
#[test]
fn other_price_account_is_rejected() {
    let mut st = market_state_with_market();
    let b = id_of(&bob());
    supply(&mut st, &alice(), 5_000_000, id_of(&alice())).unwrap();
    supply_collateral(&mut st, &bob(), 1_000_000, b).unwrap();
    let fake = AccountId::new([99; 32]);
    set_price(&mut st, fake, coll_def(), loan_def(), PRICE_ONE * 1_000, T0);
    fails_with(
        borrow(&mut st, &bob(), id_of(&bob_loan()), 900_000, fake),
        "wrong price account",
    );
}

/// D3/D4: zero price and a price for the wrong pair are rejected.
#[test]
fn zero_or_wrong_pair_price_is_rejected() {
    let mut st = market_state_with_market();
    let b = id_of(&bob());
    supply(&mut st, &alice(), 5_000_000, id_of(&alice())).unwrap();
    supply_collateral(&mut st, &bob(), 1_000_000, b).unwrap();
    set_price(&mut st, price_acc(), coll_def(), loan_def(), 0, T0);
    fails_with(
        borrow(&mut st, &bob(), id_of(&bob_loan()), 1, price_acc()),
        "oracle price is zero",
    );
    set_price(&mut st, price_acc(), loan_def(), coll_def(), PRICE_ONE, T0);
    fails_with(
        borrow(&mut st, &bob(), id_of(&bob_loan()), 1, price_acc()),
        "oracle base is not the collateral token",
    );
}

/// D5a: a price stamped further ahead than MAX_FUTURE_SKEW_MS is rejected, so a
/// future-dated price cannot stay fresh; within the skew it is accepted.
#[test]
fn future_dated_price_is_rejected() {
    let mut st = market_state_with_market();
    let b = id_of(&bob());
    supply(&mut st, &alice(), 5_000_000, id_of(&alice())).unwrap();
    supply_collateral(&mut st, &bob(), 1_000_000, b).unwrap();
    let skew = argo_lending_core::MAX_FUTURE_SKEW_MS;
    set_price(
        &mut st,
        price_acc(),
        coll_def(),
        loan_def(),
        PRICE_ONE,
        T0 + skew + 1,
    );
    fails_with(
        borrow(&mut st, &bob(), id_of(&bob_loan()), 1, price_acc()),
        "oracle price is from the future",
    );
    set_price(
        &mut st,
        price_acc(),
        coll_def(),
        loan_def(),
        PRICE_ONE,
        T0 + skew,
    );
    borrow(&mut st, &bob(), id_of(&bob_loan()), 1, price_acc()).expect("within skew");
}

/// D14: the genesis admin constant is the harness admin key, and nobody else
/// can initialise Config.
#[test]
fn only_genesis_admin_can_initialize() {
    assert_eq!(
        argo_lending_core::ADMIN_GENESIS.value(),
        id_of(&admin()).value(),
        "ADMIN_GENESIS must be the id of key [1; 32]"
    );
    let mut st = V03State::new();
    deploy(&mut st, argo_lending_methods::ARGO_LENDING_ELF);
    let cfg = config_id(&argo());
    fails_with(
        submit(
            &mut st,
            vec![cfg, id_of(&alice())],
            &[&alice()],
            &Instruction::Initialize,
        ),
        "only the genesis admin may initialize",
    );
    submit(
        &mut st,
        vec![cfg, id_of(&admin())],
        &[&admin()],
        &Instruction::Initialize,
    )
    .expect("genesis admin");
}

/// Only on_behalf itself may withdraw (manager authorisation is M3).
#[test]
fn withdraw_by_someone_else_is_rejected() {
    let mut st = market_state_with_market();
    let a = id_of(&alice());
    supply(&mut st, &alice(), 1_000_000, a).unwrap();
    fails_with(
        withdraw(&mut st, &bob_loan(), a, id_of(&alice2()), 1),
        "caller is not on_behalf",
    );
}

/// Liquidity: borrowing more than is supplied fails even with ample collateral.
#[test]
fn borrow_beyond_liquidity_is_rejected() {
    let mut st = market_state_with_market();
    let b = id_of(&bob());
    supply(&mut st, &alice(), 100_000, id_of(&alice())).unwrap();
    supply_collateral(&mut st, &bob(), 5_000_000, b).unwrap();
    fails_with(
        borrow(
            &mut st,
            &bob(),
            id_of(&bob_loan()),
            100_000 + DEAD_DEPOSIT + 1,
            price_acc(),
        ),
        "insufficient liquidity",
    );
}

/// Atomicity: a failed token leg (payer lacks funds) reverts Argo's accounting.
#[test]
fn failed_token_leg_reverts_accounting() {
    let mut st = market_state_with_market();
    let before = market_state(&st);
    fails_with(
        supply(&mut st, &bob_loan(), 1, id_of(&bob_loan())),
        "Insufficient balance",
    ); // token leg: balance 0
    assert_eq!(market_state(&st), before);
    assert_eq!(
        st.get_account_by_id(position(&id_of(&bob_loan()))),
        Account::default()
    );
}

/// R7: donating straight into the loan vault does not move share price, so a
/// later supplier is not diluted (accounting is not balance-based, and virtual
/// shares cover the empty market).
#[test]
fn donation_does_not_inflate_shares() {
    let mut st = market_state_with_market();
    let i = ids();
    // Attacker donates 1_000_000 directly to the vault through the token program.
    let msg = public_transaction::Message::try_new(
        TOKEN_PROGRAM_ID,
        vec![id_of(&alice()), i.loan_vault],
        vec![st.get_account_by_id(id_of(&alice())).nonce],
        &argo_lending_core::external::TokenInstruction::Transfer {
            amount_to_transfer: 1_000_000,
        },
    )
    .unwrap();
    let ws = public_transaction::WitnessSet::for_message(&msg, &[&alice()]);
    st.transition_from_public_transaction(&PublicTransaction::new(msg, ws), 1, T0)
        .expect("donation");
    assert_eq!(market_state(&st).total_supply_assets, DEAD_DEPOSIT);

    // Victim supplies 1 unit and can withdraw it back.
    let victim = id_of(&alice2());
    st.force_insert_account(victim, holding(loan_def(), 1));
    supply(&mut st, &alice2(), 1, victim).expect("victim supply");
    let p = position_state(&st, &victim);
    assert!(p.supply_shares > 0, "victim got shares");
    withdraw(&mut st, &alice2(), victim, id_of(&bob_loan()), 1).expect("victim withdraws in full");
}

/// F3 guards on create_market.
#[test]
fn create_market_guards() {
    let mut st = base_state();
    // Same token on both sides: the definition would be listed twice, which the
    // runtime rejects before Argo runs. Argo's own check (D12) is defence in depth.
    let mut p = params();
    p.collateral_token = loan_def();
    fails_with(create(&mut st, p, descriptor()), "Duplicate account_ids");
    // Oracle param that is not the descriptor's digest.
    let mut p = params();
    p.oracle = [0; 32];
    fails_with(
        create(&mut st, p, descriptor()),
        "oracle param is not the descriptor's digest",
    );
    // LLTV not enabled.
    let mut p = params();
    p.lltv = 1;
    fails_with(create(&mut st, p, descriptor()), "lltv not enabled");
    // max_age below one minute.
    let mut d = descriptor();
    d.max_age_ms = 59_999;
    let mut p = params();
    p.oracle = oracle_digest(&d);
    fails_with(create(&mut st, p, d), "max_age_ms out of bounds");
    // Oracle program that is the default (unowned) program id.
    let mut d = descriptor();
    d.oracle_program = [0; 8];
    let mut p = params();
    p.oracle = oracle_digest(&d);
    fails_with(
        create(&mut st, p, d),
        "oracle program must not be the default program id",
    );
    // max_age above one day.
    let mut d = descriptor();
    d.max_age_ms = 86_400_001;
    let mut p = params();
    p.oracle = oracle_digest(&d);
    fails_with(create(&mut st, p, d), "max_age_ms out of bounds");
    // Feed that does not exist (D7).
    let mut d = descriptor();
    d.price_account = AccountId::new([77; 32]);
    let mut p = params();
    p.oracle = oracle_digest(&d);
    fails_with(
        create(&mut st, p, d),
        "price account not owned by the oracle program",
    );
    // The real one works once, then never again.
    create(&mut st, params(), descriptor()).expect("create");
    fails_with(
        create(&mut st, params(), descriptor()),
        "market already exists",
    );
}

/// Accrual scaffolding (D8, D9): an operation at a later clock moves
/// `last_update_ms` forward; an earlier clock never moves it back. M1 has no
/// rate, so totals do not change.
#[test]
fn accrual_moves_last_update_forward_only() {
    let mut st = market_state_with_market();
    let a = id_of(&alice());
    set_clock(&mut st, T0 + 5_000);
    supply(&mut st, &alice(), 1_000, a).unwrap();
    let m = market_state(&st);
    assert_eq!(m.last_update_ms, T0 + 5_000);
    assert_eq!(m.total_borrow_assets, 0);
    set_clock(&mut st, T0 + 1_000);
    supply(&mut st, &alice(), 1_000, a).unwrap();
    assert_eq!(
        market_state(&st).last_update_ms,
        T0 + 5_000,
        "clock going back does not rewind"
    );
}

/// R7, near-empty market: the attacker is the first real supplier (1 unit)
/// next to the dead deposit, then donates straight into the vault. A later
/// victim still gets shares and withdraws everything back. (A truly empty
/// market cannot exist on chain because of the dead deposit; the empty case is
/// argo_core's `empty_market_first_deposit_is_not_100_percent`.)
#[test]
fn near_empty_market_attacker_cannot_dilute_victim() {
    let mut st = market_state_with_market();
    let i = ids();
    let attacker = id_of(&alice());
    supply(&mut st, &alice(), 1, attacker).unwrap();
    let msg = public_transaction::Message::try_new(
        TOKEN_PROGRAM_ID,
        vec![attacker, i.loan_vault],
        vec![st.get_account_by_id(attacker).nonce],
        &argo_lending_core::external::TokenInstruction::Transfer {
            amount_to_transfer: 5_000_000,
        },
    )
    .unwrap();
    let ws = public_transaction::WitnessSet::for_message(&msg, &[&alice()]);
    st.transition_from_public_transaction(&PublicTransaction::new(msg, ws), 1, T0)
        .expect("donation");

    let victim = id_of(&alice2());
    st.force_insert_account(victim, holding(loan_def(), 1_000));
    supply(&mut st, &alice2(), 1_000, victim).expect("victim supply");
    let shares = position_state(&st, &victim).supply_shares;
    assert!(shares > 0);
    withdraw_shares(&mut st, &alice2(), id_of(&bob_loan()), shares).expect("victim exits");
    assert_eq!(
        balance(&st, id_of(&bob_loan())),
        1_000,
        "victim gets every unit back"
    );
}

/// Rounding: supplying 1 unit and withdrawing all its shares, repeatedly, never
/// returns more than was put in.
#[test]
fn supply_withdraw_rounding_loop_never_profits() {
    let mut st = market_state_with_market();
    let me = id_of(&alice2());
    st.force_insert_account(me, holding(loan_def(), 50));
    for _ in 0..20 {
        supply(&mut st, &alice2(), 1, me).unwrap();
        let shares = position_state(&st, &me).supply_shares;
        withdraw_shares(&mut st, &alice2(), id_of(&bob_loan()), shares).unwrap();
    }
    let out = balance(&st, id_of(&bob_loan()));
    let left = balance(&st, me);
    assert!(
        out + left <= 50,
        "rounding paid out {out} + kept {left} from 50"
    );
}

/// Spec §3: a payer or receiver must be a holding of the market's token, owned
/// by the pinned token program. Argo rejects the wrong token before any token
/// call, and nothing changes.
#[test]
fn wrong_token_holding_is_rejected() {
    let mut st = market_state_with_market();
    let b = id_of(&bob());
    let before = market_state(&st);
    fails_with(
        supply(&mut st, &bob(), 1, b),
        "holding is for another token",
    );
    assert_eq!(market_state(&st), before);

    supply(&mut st, &alice(), 5_000_000, id_of(&alice())).unwrap();
    supply_collateral(&mut st, &bob(), 1_000_000, b).unwrap();
    fails_with(
        borrow(&mut st, &bob(), id_of(&bob_coll2()), 1, price_acc()),
        "holding is for another token",
    );
}

/// Submit any Argo instruction with an explicit signer set.
fn submit_as(
    st: &mut V03State,
    accounts: Vec<AccountId>,
    signers: &[&PrivateKey],
    ix: Instruction,
) -> Result<(), lee::error::LeeError> {
    submit(st, accounts, signers, &ix)
}

/// Collateralised borrower with a supplied market, used by several tests.
fn funded() -> V03State {
    let mut st = market_state_with_market();
    supply(&mut st, &alice(), 1_000_000, id_of(&alice())).unwrap();
    supply_collateral(&mut st, &bob(), 2_000_000, id_of(&bob())).unwrap();
    st
}

/// Auth: the owner must sign. Listing alice as caller without her signature is
/// stopped by the SPEL signer check before the handler runs.
#[test]
fn unsigned_caller_is_rejected() {
    let mut st = funded();
    let i = ids();
    let a = id_of(&alice());
    fails_with(
        submit_as(
            &mut st,
            vec![
                i.market,
                position(&a),
                i.loan_vault,
                id_of(&alice2()),
                a,
                CLOCK_01_ACCOUNT_ID,
            ],
            &[&alice2()],
            Instruction::Withdraw {
                assets: 1,
                shares: 0,
                on_behalf: a,
            },
        ),
        "must be a signer",
    );
}

/// Auth: admin operations need the admin's signature and only the admin.
#[test]
fn admin_operations_are_admin_only() {
    let mut st = base_state();
    let cfg = config_id(&argo());
    fails_with(
        submit_as(
            &mut st,
            vec![cfg, id_of(&alice())],
            &[&alice()],
            Instruction::EnableLltv { lltv: 1 },
        ),
        "not the admin",
    );
    fails_with(
        submit_as(
            &mut st,
            vec![cfg, id_of(&alice())],
            &[&alice()],
            Instruction::EnableIrm { irm: [5; 8] },
        ),
        "not the admin",
    );
    fails_with(
        submit_as(
            &mut st,
            vec![cfg, id_of(&admin())],
            &[&alice()],
            Instruction::EnableLltv { lltv: 1 },
        ),
        "must be a signer",
    );
    fails_with(
        submit_as(
            &mut st,
            vec![cfg, id_of(&admin())],
            &[&admin()],
            Instruction::EnableLltv {
                lltv: argo_core::WAD,
            },
        ),
        "lltv must be below 1",
    );
    fails_with(
        submit_as(
            &mut st,
            vec![cfg, id_of(&admin())],
            &[&admin()],
            Instruction::EnableLltv { lltv: LLTV },
        ),
        "lltv already enabled",
    );
    fails_with(
        submit_as(
            &mut st,
            vec![cfg, id_of(&admin())],
            &[&admin()],
            Instruction::Initialize,
        ),
        "config already initialised",
    );
}

/// D15: a plain key that no program owns cannot act as a position owner (it
/// would be dropped from Argo's output after its first signature).
#[test]
fn unowned_key_cannot_be_caller() {
    let mut st = funded();
    let k = key(30);
    let kid = id_of(&k);
    st.force_insert_account(id_of(&bob_coll2()), holding(coll_def(), 1_000_000));
    supply_collateral(&mut st, &bob_coll2(), 1_000_000, kid).expect("anyone may supply for k");
    let i = ids();
    fails_with(
        submit_as(
            &mut st,
            vec![
                i.market,
                position(&kid),
                i.loan_vault,
                id_of(&bob_loan()),
                kid,
                price_acc(),
                CLOCK_01_ACCOUNT_ID,
            ],
            &[&k],
            Instruction::Borrow {
                assets: 1,
                shares: 0,
                on_behalf: kid,
            },
        ),
        "caller must be a program-owned account",
    );
}

/// The Position must be PDA(market, on_behalf): bob cannot borrow against
/// alice's position.
#[test]
fn other_users_position_is_rejected() {
    let mut st = funded();
    let i = ids();
    let b = id_of(&bob());
    fails_with(
        submit_as(
            &mut st,
            vec![
                i.market,
                position(&id_of(&alice())),
                i.loan_vault,
                id_of(&bob_loan()),
                b,
                price_acc(),
                CLOCK_01_ACCOUNT_ID,
            ],
            &[&bob()],
            Instruction::Borrow {
                assets: 1,
                shares: 0,
                on_behalf: b,
            },
        ),
        "position id mismatch",
    );
}

/// Fake-vault substitution: the user's own holding of the right token cannot
/// stand in for a vault.
#[test]
fn own_holding_as_vault_is_rejected() {
    let mut st = funded();
    let i = ids();
    let b = id_of(&bob());
    st.force_insert_account(id_of(&bob_coll2()), holding(coll_def(), 10));
    fails_with(
        submit_as(
            &mut st,
            vec![i.market, position(&b), id_of(&bob_coll2()), b],
            &[&bob()],
            Instruction::SupplyCollateral {
                assets: 1,
                on_behalf: b,
            },
        ),
        "collateral vault id mismatch",
    );
    borrow(&mut st, &bob(), id_of(&bob_loan()), 10, price_acc()).unwrap();
    fails_with(
        submit_as(
            &mut st,
            vec![
                i.market,
                position(&b),
                id_of(&alice2()),
                id_of(&bob_loan()),
                CLOCK_01_ACCOUNT_ID,
            ],
            &[&bob_loan()],
            Instruction::Repay {
                assets: 10,
                shares: 0,
                on_behalf: b,
            },
        ),
        "loan vault id mismatch",
    );
}

/// Forged vault debit: the pinned v2.0.1 token refuses a transfer out of an
/// Argo vault that Argo did not authorise.
#[test]
fn vault_cannot_be_debited_directly() {
    let mut st = funded();
    let i = ids();
    let msg = public_transaction::Message::try_new(
        TOKEN_PROGRAM_ID,
        vec![i.loan_vault, id_of(&alice2())],
        vec![],
        &argo_lending_core::external::TokenInstruction::Transfer {
            amount_to_transfer: 1,
        },
    )
    .unwrap();
    let no_signers: &[&PrivateKey] = &[];
    let ws = public_transaction::WitnessSet::for_message(&msg, no_signers);
    fails_with(
        st.transition_from_public_transaction(&PublicTransaction::new(msg, ws), 1, T0),
        "Account 'sender' (index 0) must be a signer",
    );
}

/// Per-position guards: nobody takes out more than they put in or owe.
#[test]
fn over_withdraw_over_collateral_over_repay_are_rejected() {
    let mut st = funded();
    let b = id_of(&bob());
    fails_with(
        withdraw(
            &mut st,
            &alice(),
            id_of(&alice()),
            id_of(&alice2()),
            1_000_001,
        ),
        "insufficient supply shares",
    );
    fails_with(
        withdraw_collateral(&mut st, &bob(), id_of(&bob_coll2()), 2_000_001),
        "insufficient collateral",
    );
    borrow(&mut st, &bob(), id_of(&bob_loan()), 100, price_acc()).unwrap();
    st.force_insert_account(id_of(&bob_loan()), holding(loan_def(), 1_000));
    fails_with(
        repay(&mut st, &bob_loan(), 101, b),
        "repaying more than owed",
    );
}

/// F5: a lender cannot withdraw what has been lent out.
#[test]
fn withdraw_beyond_liquidity_is_rejected() {
    let mut st = funded();
    borrow(&mut st, &bob(), id_of(&bob_loan()), 800_000, price_acc()).unwrap();
    // Supply 1_001_000 − 300_000 = 701_000 < 800_000 borrowed.
    fails_with(
        withdraw(
            &mut st,
            &alice(),
            id_of(&alice()),
            id_of(&alice2()),
            300_000,
        ),
        "insufficient liquidity",
    );
    withdraw(
        &mut st,
        &alice(),
        id_of(&alice()),
        id_of(&alice2()),
        201_000,
    )
    .expect("exactly the free liquidity");
}

/// Exactly one of assets and shares.
#[test]
fn exactly_one_of_assets_and_shares() {
    let mut st = market_state_with_market();
    let i = ids();
    let a = id_of(&alice());
    for (assets, shares) in [(0, 0), (1, 1)] {
        fails_with(
            submit_as(
                &mut st,
                vec![i.market, position(&a), i.loan_vault, a, CLOCK_01_ACCOUNT_ID],
                &[&alice()],
                Instruction::Supply {
                    assets,
                    shares,
                    on_behalf: a,
                },
            ),
            "exactly one of assets and shares must be non-zero",
        );
    }
}

/// D13: the clock must be CLOCK_01 and must have started.
#[test]
fn clock_must_be_clock_01_and_started() {
    let mut st = market_state_with_market();
    let i = ids();
    let a = id_of(&alice());
    fails_with(
        submit_as(
            &mut st,
            vec![i.market, position(&a), i.loan_vault, a, price_acc()],
            &[&alice()],
            Instruction::Supply {
                assets: 1,
                shares: 0,
                on_behalf: a,
            },
        ),
        "clock must be CLOCK_01",
    );
    set_clock(&mut st, 0);
    fails_with(supply(&mut st, &alice(), 1, a), "clock not started");
}

/// Rounding against the user, on share-denominated paths where the ratio is
/// not exact. Market starts at A = 1_000, S = 1e9 (dead deposit).
#[test]
fn share_denominated_supply_and_withdraw_round_against_the_user() {
    let mut st = market_state_with_market();
    let me = id_of(&alice2());
    st.force_insert_account(me, holding(loan_def(), 10));
    let i = ids();
    let sub = |st: &mut V03State, ix: Instruction, accounts: Vec<AccountId>| {
        submit_as(st, accounts, &[&alice2()], ix)
    };

    // supply 1 share: pays ceil(1 × 1_001 / 1_001_000_000) = 1 asset.
    sub(
        &mut st,
        Instruction::Supply {
            assets: 0,
            shares: 1,
            on_behalf: me,
        },
        vec![
            i.market,
            position(&me),
            i.loan_vault,
            me,
            CLOCK_01_ACCOUNT_ID,
        ],
    )
    .unwrap();
    assert_eq!(balance(&st, me), 9);
    assert_eq!(position_state(&st, &me).supply_shares, 1);
    let m = market_state(&st);
    assert_eq!(
        (m.total_supply_assets, m.total_supply_shares),
        (1_001, 1_000_000_001)
    );

    // Withdrawing 1 asset needs ceil(1 × 1_001_000_001 / 1_002) = 999_002 shares.
    fails_with(
        withdraw(&mut st, &alice2(), me, id_of(&bob_loan()), 1),
        "insufficient supply shares",
    );
    // Withdrawing the 1 share pays floor(1 × 1_002 / 1_001_000_001) = 0.
    sub(
        &mut st,
        Instruction::Withdraw {
            assets: 0,
            shares: 1,
            on_behalf: me,
        },
        vec![
            i.market,
            position(&me),
            i.loan_vault,
            id_of(&bob_loan()),
            me,
            CLOCK_01_ACCOUNT_ID,
        ],
    )
    .unwrap();
    assert_eq!(balance(&st, id_of(&bob_loan())), 0);
    assert_eq!(position_state(&st, &me).supply_shares, 0);
}

/// Borrow by shares rounds the payout down; repay by assets burns shares rounded
/// down; repaying the rest by shares rounds the cost up and hits Morpho's
/// zeroFloorSub. Full repayment brings the position's debt to zero (F7).
#[test]
fn share_denominated_borrow_and_full_repay() {
    let mut st = funded();
    let b = id_of(&bob());
    let i = ids();
    // Empty borrow side: 1_500_000 shares pay floor(1_500_000 × 1 / 1_000_000) = 1.
    submit_as(
        &mut st,
        vec![
            i.market,
            position(&b),
            i.loan_vault,
            id_of(&bob_loan()),
            b,
            price_acc(),
            CLOCK_01_ACCOUNT_ID,
        ],
        &[&bob()],
        Instruction::Borrow {
            assets: 0,
            shares: 1_500_000,
            on_behalf: b,
        },
    )
    .unwrap();
    assert_eq!(balance(&st, id_of(&bob_loan())), 1);
    let m = market_state(&st);
    assert_eq!(
        (m.total_borrow_assets, m.total_borrow_shares),
        (1, 1_500_000)
    );

    // Repay 1 asset: burns floor(1 × 2_500_000 / 2) = 1_250_000 shares.
    repay(&mut st, &bob_loan(), 1, b).unwrap();
    assert_eq!(position_state(&st, &b).borrow_shares, 250_000);
    let m = market_state(&st);
    assert_eq!((m.total_borrow_assets, m.total_borrow_shares), (0, 250_000));

    // Repay the last 250_000 shares: costs ceil(250_000 × 1 / 1_250_000) = 1, and
    // total_borrow_assets floors at 0 instead of underflowing.
    st.force_insert_account(id_of(&bob_loan()), holding(loan_def(), 1));
    submit_as(
        &mut st,
        vec![
            i.market,
            position(&b),
            i.loan_vault,
            id_of(&bob_loan()),
            CLOCK_01_ACCOUNT_ID,
        ],
        &[&bob_loan()],
        Instruction::Repay {
            assets: 0,
            shares: 250_000,
            on_behalf: b,
        },
    )
    .unwrap();
    assert_eq!(balance(&st, id_of(&bob_loan())), 0);
    assert_eq!(position_state(&st, &b).borrow_shares, 0);
    let m = market_state(&st);
    assert_eq!((m.total_borrow_assets, m.total_borrow_shares), (0, 0));
}

/// F1 singleton, R1 isolation: two markets on one program; one market's vault
/// cannot be used for the other.
#[test]
fn two_markets_are_isolated() {
    let mut st = base_state();
    let lltv2 = 500_000_000_000_000_000;
    submit_as(
        &mut st,
        vec![config_id(&argo()), id_of(&admin())],
        &[&admin()],
        Instruction::EnableLltv { lltv: lltv2 },
    )
    .unwrap();
    create(&mut st, params(), descriptor()).unwrap();
    let mut p2 = params();
    p2.lltv = lltv2;
    create(&mut st, p2, descriptor()).unwrap();
    let mid2 = market_id(&p2);
    let a = id_of(&alice());
    let i = ids();
    fails_with(
        submit_as(
            &mut st,
            vec![
                i.market,
                position(&a),
                loan_vault_id(&argo(), &mid2),
                a,
                CLOCK_01_ACCOUNT_ID,
            ],
            &[&alice()],
            Instruction::Supply {
                assets: 1,
                shares: 0,
                on_behalf: a,
            },
        ),
        "loan vault id mismatch",
    );
    supply(&mut st, &alice(), 1_000, a).unwrap();
    assert_eq!(balance(&st, i.loan_vault), DEAD_DEPOSIT + 1_000);
    assert_eq!(balance(&st, loan_vault_id(&argo(), &mid2)), DEAD_DEPOSIT);
    assert_eq!(st.get_account_by_id(i.market).program_owner, argo());
    assert_eq!(
        st.get_account_by_id(i.loan_vault).program_owner,
        TOKEN_PROGRAM_ID
    );
}

/// F3 guards the earlier test does not reach: IRM not enabled, unknown
/// descriptor version, the one-day ceiling accepted, definitions that do not
/// match the params, and a non-fungible definition.
#[test]
fn create_market_more_guards() {
    let mut st = base_state();
    let mut p = params();
    p.irm = [8; 8];
    fails_with(create(&mut st, p, descriptor()), "irm not enabled");

    let mut d = descriptor();
    d.version = 2;
    let mut p = params();
    p.oracle = oracle_digest(&d);
    fails_with(create(&mut st, p, d), "unknown oracle descriptor version");

    // Accounts listing the wrong definition for the loan token.
    let mut accounts = create_market_accounts(&params(), &descriptor());
    accounts[5] = AccountId::new([55; 32]);
    st.force_insert_account(AccountId::new([55; 32]), definition("OTHER"));
    fails_with(
        submit(
            &mut st,
            accounts,
            &[&alice()],
            &Instruction::CreateMarket {
                params: params(),
                oracle: descriptor(),
            },
        ),
        "wrong loan definition",
    );

    // A non-fungible collateral definition.
    let nft = AccountId::new([56; 32]);
    st.force_insert_account(
        nft,
        Account {
            program_owner: TOKEN_PROGRAM_ID,
            balance: 0,
            data: Data::try_from(
                borsh::to_vec(&TokenDefinition::NonFungible {
                    name: "NFT".to_owned(),
                    printable_supply: 1,
                    metadata_id: AccountId::new([57; 32]),
                })
                .unwrap(),
            )
            .unwrap(),
            nonce: Nonce(0),
        },
    );
    set_price(
        &mut st,
        AccountId::new([58; 32]),
        nft,
        loan_def(),
        PRICE_ONE,
        T0,
    );
    let d = OracleDescriptor {
        price_account: AccountId::new([58; 32]),
        ..descriptor()
    };
    let mut p = params();
    p.collateral_token = nft;
    p.oracle = oracle_digest(&d);
    fails_with(create(&mut st, p, d), "token definition is not fungible");

    // The one-day ceiling itself is accepted.
    let d = OracleDescriptor {
        max_age_ms: 86_400_000,
        ..descriptor()
    };
    let mut p = params();
    p.oracle = oracle_digest(&d);
    create(&mut st, p, d).expect("max_age_ms = one day");
}

/// Rollback: a creator who cannot pay the dead deposit fails in the third chained
/// leg, and the market, both vaults and the dead position stay unclaimed, so the
/// market can still be created later.
#[test]
fn create_market_rolls_back_when_dead_deposit_fails() {
    let mut st = base_state();
    st.force_insert_account(id_of(&alice()), holding(loan_def(), DEAD_DEPOSIT - 1));
    fails_with(
        create(&mut st, params(), descriptor()),
        "Insufficient balance",
    );
    for id in create_market_accounts(&params(), &descriptor())
        .into_iter()
        .skip(1)
        .take(4)
    {
        assert_eq!(
            st.get_account_by_id(id),
            Account::default(),
            "{id:?} stayed unclaimed"
        );
    }
    st.force_insert_account(id_of(&alice()), holding(loan_def(), START));
    create(&mut st, params(), descriptor()).expect("retry succeeds");
}

/// Withdrawing, withdrawing collateral, borrowing or repaying needs an existing
/// position.
#[test]
fn operations_on_a_missing_position_are_rejected() {
    let mut st = funded();
    fails_with(
        withdraw(&mut st, &alice2(), id_of(&alice2()), id_of(&bob_loan()), 1),
        "no position",
    );
    fails_with(
        withdraw_collateral(&mut st, &alice2(), id_of(&bob_coll2()), 1),
        "no position",
    );
    fails_with(
        borrow(&mut st, &alice2(), id_of(&bob_loan()), 1, price_acc()),
        "no position",
    );
    fails_with(
        repay(&mut st, &bob_loan(), 1, id_of(&alice2())),
        "no position",
    );
}

/// D15 through the real runtime: a plain key that has signed before (nonce 1,
/// still unowned) is exactly the input SPEL would drop from the output, so the
/// guard refuses it as caller and as payer before anything else runs.
#[test]
fn used_unowned_key_is_refused_by_the_runtime_check() {
    let mut st = funded();
    let k = key(31);
    let kid = id_of(&k);
    st.force_insert_account(
        kid,
        Account {
            nonce: Nonce(1),
            ..Account::default()
        },
    );
    let before = market_state(&st);
    fails_with(
        withdraw(&mut st, &k, kid, id_of(&alice2()), 1),
        "is unowned and not default, so the runtime cannot check it",
    );
    fails_with(
        supply(&mut st, &k, 1, kid),
        "is unowned and not default, so the runtime cannot check it",
    );
    assert_eq!(market_state(&st), before);
}

/// F13: owner and quote are checked on every price read, not only at creation.
#[test]
fn price_account_owner_and_quote_are_checked_on_read() {
    let mut st = funded();
    set_price(
        &mut st,
        price_acc(),
        coll_def(),
        AccountId::new([77; 32]),
        PRICE_ONE,
        T0,
    );
    fails_with(
        borrow(&mut st, &bob(), id_of(&bob_loan()), 1, price_acc()),
        "oracle quote is not the loan token",
    );
    set_price(&mut st, price_acc(), coll_def(), loan_def(), PRICE_ONE, T0);
    let mut moved = st.get_account_by_id(price_acc());
    moved.program_owner = [8; 8];
    st.force_insert_account(price_acc(), moved);
    fails_with(
        borrow(&mut st, &bob(), id_of(&bob_loan()), 1, price_acc()),
        "price account not owned by the oracle program",
    );
}

/// D15: a receiver must be a holding owned by the pinned token program; a
/// fresh account is refused before any token call.
#[test]
fn receiver_not_owned_by_the_token_program_is_rejected() {
    let mut st = funded();
    let before = market_state(&st);
    fails_with(
        borrow(&mut st, &bob(), id_of(&key(32)), 1, price_acc()),
        "holding not owned by the pinned token program",
    );
    assert_eq!(market_state(&st), before);
}
