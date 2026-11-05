//! Spike S-D: per-execution cycle counts, read straight from the risc0
//! executor with the exact inputs the LEZ state machine writes
//! (`lee::program::Program::write_inputs` order: self id, caller id,
//! pre-states, instruction words). `RISC0_DEV_MODE` is irrelevant here: the
//! executor runs the guest for real, it just does not prove.
//!
//! Output is a table on stderr plus a `VERDICT S-D:` line; run with
//! `-- --nocapture`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::as_conversions,
    reason = "spike"
)]

use lee_core::account::{Account, AccountId, AccountWithMetadata, Data, Nonce};
use lee_core::program::ProgramId;
use risc0_zkvm::{default_executor, ExecutorEnv};
use spike_vault_core::{state_id, vault_id, Instruction, VaultState};
use token_core::TokenHolding;

const BUDGET: u64 = 32 * 1024 * 1024;

fn spike_id() -> ProgramId {
    spike_vault_methods::SPIKE_VAULT_ID
}
fn token_id() -> ProgramId {
    programs::token().id()
}
fn definition_id() -> AccountId {
    AccountId::new([5; 32])
}

#[derive(Debug)]
struct Run {
    /// Sum of segment cycles (po2-padded, what proving time scales with).
    padded: u64,
    segments: usize,
}

fn try_execute(
    elf: &[u8],
    program_id: ProgramId,
    caller: Option<ProgramId>,
    pre: Vec<AccountWithMetadata>,
    ix: impl serde::Serialize,
) -> Result<Run, String> {
    let words = risc0_zkvm::serde::to_vec(&ix).unwrap();
    let mut b = ExecutorEnv::builder();
    b.session_limit(Some(BUDGET));
    b.write(&program_id).unwrap();
    b.write(&caller).unwrap();
    b.write(&pre).unwrap();
    b.write(&words).unwrap();
    let env = b.build().unwrap();
    let info = default_executor()
        .execute(env, elf)
        .map_err(|e| e.to_string())?;
    Ok(Run {
        padded: info.segments.iter().map(|s| u64::from(s.cycles)).sum(),
        segments: info.segments.len(),
    })
}
fn execute(
    elf: &[u8],
    program_id: ProgramId,
    caller: Option<ProgramId>,
    pre: Vec<AccountWithMetadata>,
    ix: impl serde::Serialize,
) -> Run {
    try_execute(elf, program_id, caller, pre, ix).expect("guest executes within budget")
}

fn state_account(ops: u64, pad_len: usize) -> AccountWithMetadata {
    let st = VaultState {
        ops,
        internal_hits: 0,
        vault: vault_id(&spike_id()),
        pad: vec![7u8; pad_len],
    };
    AccountWithMetadata {
        account: Account {
            program_owner: spike_id(),
            balance: 0,
            data: Data::try_from(borsh::to_vec(&st).unwrap()).unwrap(),
            nonce: Nonce(0),
        },
        is_authorized: false,
        account_id: state_id(&spike_id()),
    }
}
fn holding(id: AccountId, balance: u128, authorized: bool) -> AccountWithMetadata {
    AccountWithMetadata {
        account: Account {
            program_owner: token_id(),
            balance: 0,
            data: Data::from(&TokenHolding::Fungible {
                definition_id: definition_id(),
                balance,
            }),
            nonce: Nonce(0),
        },
        is_authorized: authorized,
        account_id: id,
    }
}

fn row(name: &str, r: &Run) {
    eprintln!(
        "{name:<44} {:>12} cycles  {:>2} seg  {:>5.1}% of 32M",
        r.padded,
        r.segments,
        100.0 * r.padded as f64 / BUDGET as f64
    );
}

#[test]
fn s_d_cycle_table() {
    let elf = spike_vault_methods::SPIKE_VAULT_ELF;
    let user = AccountId::new([42; 32]);
    let recipient = AccountId::new([43; 32]);
    eprintln!("\n== S-D per-execution cycles (spike_vault guest, builtin token) ==");

    // D1 supply-shaped: Argo leg (state write + 1 chained transfer emitted) and the token leg.
    let argo_leg = execute(
        elf,
        spike_id(),
        None,
        vec![
            state_account(1, 0),
            holding(user, 1_000_000, true),
            holding(vault_id(&spike_id()), 0, false),
        ],
        Instruction::PayIn { amount: 1_000 },
    );
    row("D1 PayIn: Argo leg", &argo_leg);
    let token_leg = execute(
        programs::token().elf(),
        token_id(),
        Some(spike_id()),
        vec![
            holding(user, 1_000_000, true),
            holding(vault_id(&spike_id()), 0, false),
        ],
        token_core::Instruction::Transfer {
            amount_to_transfer: 1_000,
        },
    );
    row("D1 token Transfer leg", &token_leg);

    // D2 settlement-shaped: Argo leg emitting 2 chained calls (one token transfer + one self-call).
    // This is NOT a liquidation; it is the heaviest instruction shape the M0 spike program has.
    let pay_out = execute(
        elf,
        spike_id(),
        None,
        vec![
            state_account(1, 0),
            holding(vault_id(&spike_id()), 500_000, false),
            holding(recipient, 0, false),
        ],
        Instruction::PayOut {
            amount: 1_000,
            then_fail: false,
            wrong_seed: false,
        },
    );
    row("D2 PayOut: Argo leg (2 chained calls)", &pay_out);
    let internal = execute(
        elf,
        spike_id(),
        Some(spike_id()),
        vec![state_account(2, 0)],
        Instruction::Internal { must_fail: false },
    );
    row("D2 Internal self-call leg", &internal);

    // D2b math: cycles per iteration of accrual+health+LIF-shaped arithmetic.
    let base = execute(
        elf,
        spike_id(),
        None,
        vec![state_account(1, 0)],
        Instruction::Stress { iters: 0, pad: 0 },
    );
    row("D2b Stress iters=0 (baseline)", &base);
    let mut per_iter: u64 = 0;
    for iters in [100u32, 1_000, 3_000, 6_000] {
        match try_execute(
            elf,
            spike_id(),
            None,
            vec![state_account(1, 0)],
            Instruction::Stress { iters, pad: 0 },
        ) {
            Ok(r) => {
                row(&format!("D2b Stress iters={iters}"), &r);
                per_iter = r.padded.saturating_sub(base.padded) / u64::from(iters);
            }
            Err(e) => eprintln!(
                "{:<44} over budget: {e}",
                format!("D2b Stress iters={iters}")
            ),
        }
    }
    eprintln!(
        "{:<44} {per_iter:>12.0} cycles/iter",
        "D2b marginal cost per math iteration"
    );
    // D2c: the runtime's per-execution limit is real and is the padded metric.
    let over = try_execute(
        elf,
        spike_id(),
        None,
        vec![state_account(1, 0)],
        Instruction::Stress {
            iters: 10_000,
            pad: 0,
        },
    )
    .expect_err("D2c: 10k iterations (~52M padded cycles) must exceed the 32M session limit");
    assert!(
        over.contains("Session limit exceeded"),
        "D2c: expected the executor's session-limit error, got: {over}"
    );
    eprintln!("{:<44} rejected: {over}", "D2c Stress iters=10000");

    // D3 account-size sweep: pre-state carries `pad` bytes; post-state keeps them.
    let mut last = base.padded;
    for pad in [1_024u32, 10_240, 51_200] {
        match try_execute(
            elf,
            spike_id(),
            None,
            vec![state_account(1, pad as usize)],
            Instruction::Stress { iters: 0, pad },
        ) {
            Ok(r) => {
                row(&format!("D3 Stress pad={pad}B"), &r);
                last = r.padded;
            }
            Err(e) => eprintln!("{:<44} over budget: {e}", format!("D3 Stress pad={pad}B")),
        }
    }
    // Sum of the legs that actually execute for PayOut: Argo leg + one token Transfer + one self-call.
    let chain_total = pay_out.padded + token_leg.padded + internal.padded;
    eprintln!("{:<44} {chain_total:>12} cycles (sum of the three legs; relevant under a future tx-wide budget)", "D2 PayOut chain total");
    // Extrapolation for a real liquidation (M3 measures it): PayOut-shaped Argo leg + ~40 math iterations
    // (accrual, health, LIF, seize/repay derivation) + a second token transfer.
    let liquidation_estimate = pay_out.padded + 40 * per_iter + token_leg.padded;
    eprintln!("{:<44} {liquidation_estimate:>12} cycles (estimate, not measured: PayOut leg + 40 math iterations + 1 more transfer)", "D2 liquidation extrapolation");

    let go =
        pay_out.padded < BUDGET / 2 && chain_total < BUDGET && liquidation_estimate < BUDGET / 2;
    let verdict = if go {
        "GO"
    } else if pay_out.padded < BUDGET {
        "PARTIAL"
    } else {
        "NO-GO"
    };
    eprintln!("VERDICT S-D: {verdict} -- heaviest measured leg {} cycles ({:.1}% of 32M); PayOut chain {} cycles; liquidation extrapolation {} cycles; {} cycles/math-iteration; carrying a 50KB account costs {} cycles", pay_out.padded, 100.0 * pay_out.padded as f64 / BUDGET as f64, chain_total, liquidation_estimate, per_iter, last);
    assert!(
        go,
        "S-D GO condition failed: heaviest leg {} / chain {} / extrapolation {}",
        pay_out.padded, chain_total, liquidation_estimate
    );
}
