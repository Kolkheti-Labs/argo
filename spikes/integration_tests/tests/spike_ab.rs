//! Spikes S-A (chained-call atomicity + re-entry) and S-B (custody rule),
//! run against the in-process LEZ v0.2.4 state machine with the BUILTIN token
//! program (the one the public testnet runs). Every test states which spike
//! item it decides. `spikes/run.sh S-A|S-B` runs these and prints the verdict.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, reason = "tests")]

use lee::{
    program_deployment_transaction, public_transaction, PrivateKey, ProgramDeploymentTransaction,
    PublicKey, PublicTransaction, V03State,
};
use lee_core::account::{Account, AccountId, Data, Nonce};
use lee_core::program::ProgramId;
use spike_vault_core::{state_id, vault_id, Instruction, VaultState};
use token_core::{TokenDefinition, TokenHolding};

const USER_INIT: u128 = 1_000_000;
const PAY_IN: u128 = 400_000;
const PAY_OUT: u128 = 150_000;

fn key(b: u8) -> PrivateKey {
    PrivateKey::try_new([b; 32]).expect("valid key")
}
fn id_of(k: &PrivateKey) -> AccountId {
    AccountId::from(&PublicKey::new_from_private_key(k))
}
fn user_key() -> PrivateKey {
    key(42)
}
fn recipient_key() -> PrivateKey {
    key(43)
}
fn definition_id() -> AccountId {
    AccountId::new([5; 32])
}
fn token_id() -> ProgramId {
    programs::token().id()
}
fn spike_id() -> ProgramId {
    spike_vault_methods::SPIKE_VAULT_ID
}

fn holding(balance: u128) -> Account {
    Account {
        program_owner: token_id(),
        balance: 0,
        data: Data::from(&TokenHolding::Fungible { definition_id: definition_id(), balance }),
        nonce: Nonce(0),
    }
}

fn fresh_state() -> V03State {
    let mut st = V03State::new();
    for elf in [programs::token().elf().to_vec(), spike_vault_methods::SPIKE_VAULT_ELF.to_vec()] {
        st.transition_from_program_deployment_transaction(&ProgramDeploymentTransaction::new(
            program_deployment_transaction::Message::new(elf),
        ))
        .expect("deploy");
    }
    st.force_insert_account(
        definition_id(),
        Account {
            program_owner: token_id(),
            balance: 0,
            data: Data::from(&TokenDefinition::Fungible {
                name: "SPK".to_owned(),
                total_supply: USER_INIT,
                metadata_id: None,
            }),
            nonce: Nonce(0),
        },
    );
    st.force_insert_account(id_of(&user_key()), holding(USER_INIT));
    st.force_insert_account(id_of(&recipient_key()), holding(0));
    st
}

fn nonce(st: &V03State, id: AccountId) -> Nonce {
    st.get_account_by_id(id).nonce
}

fn submit(
    st: &mut V03State,
    program: ProgramId,
    accounts: Vec<AccountId>,
    signers: &[&PrivateKey],
    ix: impl serde::Serialize,
) -> Result<(), lee::error::LeeError> {
    let nonces = signers.iter().map(|k| nonce(st, id_of(k))).collect();
    let msg = public_transaction::Message::try_new(program, accounts, nonces, ix)?;
    let ws = public_transaction::WitnessSet::for_message(&msg, signers);
    st.transition_from_public_transaction(&PublicTransaction::new(msg, ws), 0, 0)
}

fn vault_state(st: &V03State) -> VaultState {
    borsh::from_slice(st.get_account_by_id(state_id(&spike_id())).data.as_ref()).expect("state")
}
fn token_balance(st: &V03State, id: AccountId) -> u128 {
    spike_vault_program::holding_balance(&st.get_account_by_id(id)).expect("fungible")
}

fn init(st: &mut V03State) -> Result<(), lee::error::LeeError> {
    submit(
        st,
        spike_id(),
        vec![state_id(&spike_id()), vault_id(&spike_id()), definition_id()],
        &[],
        Instruction::Init,
    )
}
fn pay_in(st: &mut V03State, amount: u128) -> Result<(), lee::error::LeeError> {
    submit(
        st,
        spike_id(),
        vec![state_id(&spike_id()), id_of(&user_key()), vault_id(&spike_id())],
        &[&user_key()],
        Instruction::PayIn { amount },
    )
}
fn pay_out(
    st: &mut V03State,
    amount: u128,
    then_fail: bool,
    wrong_seed: bool,
) -> Result<(), lee::error::LeeError> {
    submit(
        st,
        spike_id(),
        vec![state_id(&spike_id()), vault_id(&spike_id()), id_of(&recipient_key())],
        &[],
        Instruction::PayOut { amount, then_fail, wrong_seed },
    )
}

fn funded_state() -> V03State {
    let mut st = fresh_state();
    init(&mut st).expect("B1/A1: Init claims state PDA + vault via chained InitializeAccount");
    pay_in(&mut st, PAY_IN).expect("B1: anyone can credit the program-owned vault");
    st
}

// ---------------------------------------------------------------- S-A / S-B

/// A1 + B1 + B3: happy path. Init claims both PDAs, a user credits the vault,
/// the program debits it under PDA seed and re-enters itself.
#[test]
fn a1_b1_b3_happy_path_debit_by_seed_and_self_reentry() {
    let mut st = funded_state();
    assert_eq!(st.get_account_by_id(vault_id(&spike_id())).program_owner, token_id());
    assert_eq!(st.get_account_by_id(state_id(&spike_id())).program_owner, spike_id());
    assert_eq!(token_balance(&st, vault_id(&spike_id())), PAY_IN);
    assert_eq!(vault_state(&st).ops, 1);

    pay_out(&mut st, PAY_OUT, false, false).expect("A1: PDA-authorised debit + self-call");
    let s = vault_state(&st);
    assert_eq!(s.ops, 2, "state leg applied");
    assert_eq!(s.internal_hits, 1, "A3: chained self-call reached Internal");
    assert_eq!(token_balance(&st, vault_id(&spike_id())), PAY_IN - PAY_OUT);
    assert_eq!(token_balance(&st, id_of(&recipient_key())), PAY_OUT);
}

/// A2: a failing LAST leg (self-call panics) reverts the earlier token
/// transfer AND the program's own state write. Nothing is applied.
#[test]
fn a2_late_leg_failure_reverts_everything() {
    let mut st = funded_state();
    let before = (vault_state(&st), token_balance(&st, vault_id(&spike_id())), token_balance(&st, id_of(&recipient_key())));
    let err = pay_out(&mut st, PAY_OUT, true, false).expect_err("A2: chain must fail");
    eprintln!("A2 error surfaced to the executor: {err:?}");
    let after = (vault_state(&st), token_balance(&st, vault_id(&spike_id())), token_balance(&st, id_of(&recipient_key())));
    assert_eq!(before, after, "A2: no partial commit");
}

/// A3 negative: `Internal` submitted top-level is rejected (caller_program_id
/// is the default id, not self).
#[test]
fn a3_internal_is_rejected_at_top_level() {
    let mut st = funded_state();
    let before = vault_state(&st);
    submit(&mut st, spike_id(), vec![state_id(&spike_id())], &[], Instruction::Internal { must_fail: false })
        .expect_err("A3: top-level call into an internal entrypoint must fail");
    assert_eq!(vault_state(&st), before);
}

/// A4: debit authorised with a seed that does not derive the vault is rejected.
#[test]
fn a4_wrong_pda_seed_is_rejected() {
    let mut st = funded_state();
    let before = token_balance(&st, vault_id(&spike_id()));
    let err = pay_out(&mut st, PAY_OUT, false, true).expect_err("A4: wrong seed");
    eprintln!("A4 error: {err:?}");
    assert_eq!(token_balance(&st, vault_id(&spike_id())), before);
}

/// A5 / B4: a user-signed plain token `Transfer` out of the vault is rejected:
/// the vault is not a signer and no program authorised it.
#[test]
fn a5_b4_forged_vault_debit_is_rejected() {
    let mut st = funded_state();
    let before = token_balance(&st, vault_id(&spike_id()));
    let err = submit(
        &mut st,
        token_id(),
        vec![vault_id(&spike_id()), id_of(&recipient_key())],
        &[&recipient_key()],
        token_core::Instruction::Transfer { amount_to_transfer: PAY_OUT },
    )
    .expect_err("A5: vault must not be debitable without PDA authority");
    eprintln!("A5 error: {err:?}");
    assert_eq!(token_balance(&st, vault_id(&spike_id())), before);
}

/// A6: 10 chained self-calls are accepted, 11 are rejected.
#[test]
fn a6_chain_length_limit_is_ten() {
    let mut st = funded_state();
    submit(&mut st, spike_id(), vec![state_id(&spike_id())], &[], Instruction::Fanout { n: 10 })
        .expect("A6: 10 chained calls fit");
    assert_eq!(vault_state(&st).internal_hits, 10);
    let err = submit(&mut st, spike_id(), vec![state_id(&spike_id())], &[], Instruction::Fanout { n: 11 })
        .expect_err("A6: 11 chained calls exceed MAX_NUMBER_CHAINED_CALLS");
    eprintln!("A6 error: {err:?}");
    assert_eq!(vault_state(&st).internal_hits, 10, "rejected chain applied nothing");
}

/// B2: crediting a never-initialised vault PDA by plain transfer is rejected
/// (the token program would claim it `Authorized`, which a PDA cannot be at
/// top level). Claim-first ordering is mandatory for `create_market`.
#[test]
fn b2_credit_before_claim_is_rejected_then_init_succeeds() {
    let mut st = fresh_state();
    let err = submit(
        &mut st,
        token_id(),
        vec![id_of(&user_key()), vault_id(&spike_id())],
        &[&user_key()],
        token_core::Instruction::Transfer { amount_to_transfer: PAY_IN },
    )
    .expect_err("B2: plain transfer into an unclaimed PDA must fail");
    eprintln!("B2 error: {err:?}");
    assert_eq!(st.get_account_by_id(vault_id(&spike_id())), Account::default(), "PDA untouched");
    init(&mut st).expect("B2: Init after the failed credit still claims the vault");
    pay_in(&mut st, PAY_IN).expect("B2: credit after claim succeeds");
    assert_eq!(token_balance(&st, vault_id(&spike_id())), PAY_IN);
}
