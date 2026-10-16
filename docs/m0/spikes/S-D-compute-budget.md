# S-D — per-transaction compute budget

## Question
Do a supply-shaped and a liquidate-shaped Argo instruction fit inside one
program execution's cycle limit with a known margin, and what does account
size do to that margin?

## What the source says
- `MAX_NUM_CYCLES_PUBLIC_EXECUTION = 32 * 1024 * 1024` (32M cycles) applied
  as `session_limit` per program execution, so each chained call gets its own
  32M (`lee/state_machine/src/program/mod.rs:13-15,63`). Comment says it becomes
  variable "when fees are implemented".
- Account size drives cost: lez-programs' benchmark notes ~33M extra cycles for
  a program-owned 100 KB account (paging/hashing). `DATA_MAX_LENGTH` is
  100 KiB. Keep every Argo account small.
- Other limits: 10 chained calls (public path allows 11 executions, private
  path 10), block 1 MiB, RPC body 10 MiB, no cap on accounts per tx (open
  issue 622). v0.2.4 has no fee and no `Message.fee` field.
- Upstream baselines (`docs/benchmarks/cycle_bench.md`): token Transfer
  127,726 cycles; AMM SwapExactInput 508,904; AddLiquidity 643,464; proving
  ≈100 µs/cycle po2-padded on an M2 Pro; ≈53 s per extra chained call.
- Upstream fees (PR 801: open on `dev` when this spike was written on
  2026-09-02; merged 2026-09-03, in v0.2.5-rc1 and v0.3.0, not in v0.2.4).
  In v0.3.0 one cycle budget is shared by every call in the chain, not per
  leg (`lee/state_machine/src/validated_state_diff/mod.rs:95-97`). Argo's M3
  benchmark must report both the per-leg max (v0.2.4 rule) and the chain
  total (later rule).
- LP-0002 measured on v0.2.4: a small guest ~1M cycles; the outer privacy
  circuit 4.7–5.2M cycles and >6 GB RAM to prove. Public txs are proven by
  the sequencer, private ones client-side.

## Experiment
What was actually measured (`spikes/integration_tests/tests/spike_d.rs`,
driving the `spike_vault` guest and the builtin token through risc0's
executor with the exact inputs the state machine writes):
- D1 supply-shaped: `PayIn` (state write + one emitted Transfer) as the Argo
  leg, and the token `Transfer` leg, each measured separately.
- D2 settlement-shaped: `PayOut` (state write + one Transfer + one self-call)
  and the `Internal` self-call leg. This is not a liquidation; no accrual,
  health check, LIF, or oracle read exists in M0.
- D2b math cost: `Stress { iters }` runs accrual/health/LIF-shaped arithmetic
  from `argo_core` and `irm_core` (widening mul_div in both rounding modes,
  share conversions, utilisation) for 100/1000/3000/6000 iterations to get a
  per-iteration cost; 10,000 iterations must fail with the executor's
  `Session limit exceeded` (D2c).
- D3 account-size sweep: `Stress { iters: 0, pad }` with the state account
  carrying 1 KB / 10 KB / 50 KB of ballast. The handler regenerates the pad
  byte-by-byte, so the figure is "carrying an account of that size through
  one execution", not pure paging.
- A liquidation extrapolation is printed (PayOut leg + 40 math iterations +
  one more transfer); it is an estimate, and M3 measures the real thing.
- Not done in M0: a 10-call chain overhead measurement and a real proof at
  `RISC0_DEV_MODE=0`.

## Observable
Cycles per execution (po2-padded segment sums, the same metric the runtime's
32M `session_limit` uses, as D2c shows). The test asserts GO iff the heaviest
measured leg < 16M, the PayOut chain total (three legs) < 32M, and the
liquidation extrapolation < 16M. Anything else is PARTIAL or NO-GO. PARTIAL if 16M–32M (fits, but M3 must
budget carefully). NO-GO if D2 > 32M.

## If no-go
- Two-phase liquidation via self-call (phase 1: price + reserve; phase 2:
  settle + bad debt), each phase under its own 32M. Already in the proposal
  as the funded fallback (§7).
- If even the split does not fit: shrink accounts (split Market totals from
  Market params), move LIF/accrual to fixed-point tables, and report the
  reduced max-market figure honestly in M3/M7.
