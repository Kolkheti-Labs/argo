# M0 spike plan

The five spikes are the point of M0. Each one tests an assumption that M1–M3
are designed on. "Go/no-go" is literal: every spike ends in a written verdict
in `docs/m0/verdicts.md`, and a no-go must carry the design change M1–M3 will
adopt before M1 starts.

One file per spike under `docs/m0/spikes/`, each with the same four sections:

1. **Question** — the single assumption being tested, stated so it can be false.
2. **Experiment** — what runs, on what (standalone sequencer, `RISC0_DEV_MODE=1`
   unless the question is about proving cost), with the exact program and
   client code under `spikes/`.
3. **Observable** — the concrete output that decides go/no-go, and the
   threshold. No judgement calls at verdict time.
4. **If no-go** — what changes in M1–M3, and what it costs.

| Spike | Assumption under test | Design that depends on it |
| --- | --- | --- |
| S-A | A `ChainedCall` chain is all-or-nothing: a failed later leg reverts earlier state writes; a callee can tail-call back into the caller | Every settlement path (§4), flash loans (§8), two-phase liquidation fallback (§7) |
| S-B | A program-owned token holding can be credited by anyone and debited only under the owning program's PDA authorisation, on v0.2.4 as shipped | Vault custody (§4), liquidator/fee payout atomicity |
| S-C | LP-0012 gives a structured, client-readable event surface on v0.2.4 | Reference liquidator + risk monitor observation layer (§14) |
| S-D | A supply-shaped and a liquidate-shaped instruction fit the per-tx compute envelope, with a known margin | Liquidation in one tx vs two-phase (§7), max market count (P2) |
| S-E | An ephemeral public account can be funded from a private account and pay the fee of its next public tx from that same deshield, as one user action | Private path, U9/U10 (§12) |

Section numbers refer to the accepted proposal.

## Method rules

- Every spike is a runnable: `spikes/run.sh S-X` builds, boots the harness,
  executes, and prints the observable. The verdict quotes that output.
- Negative cases are first-class. A spike that only shows the happy path is
  not finished.
- State is read over JSON-RPC (`getAccount`, `getTransaction`, `getBlock`),
  never through wallet CLI output, and dependent steps poll state rather than
  sleep.
- Where the answer is "the runtime does not do this today", the verdict says
  so and names the upstream item (LP-0012, LP-0013, LP-0015) rather than
  working around it silently.

## Order

S-A and S-B first, they share a program (a vault owner that chains into the
transfer program). S-D reuses their guests for cycle counts. S-C and S-E are
independent and can run in parallel with the others.

## Hardware note

Cycle counts (S-D) run at `RISC0_DEV_MODE=1` and read the executor's reported
cycles, which do not need a prover. Any real-proof timing uses hetzner2
(8 vCPU / 15 GB) via the 80 GB volume; the outer privacy prover needs more
than 6 GB RAM and OOMs on a 3.8 GB host.
