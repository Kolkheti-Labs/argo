# M0 spike verdicts

Each row gives the verdict from `spikes/run.sh S-X`, where it ran, and the
design consequence. Rows were corrected on 2026-10-16 after the first Logos
review (see "Corrections" below).

| Spike | Verdict | Run | Consequence for M1–M3 |
| --- | --- | --- | --- |
| S-A chained-call atomicity + re-entry | **GO** | hetzner2 2026-09-02 and CI at `566fb61` (8 tests then, incl. the since-withdrawn b5); re-run on hetzner2 2026-10-16 after removing b5, `cargo test -p spike_integration_tests --test spike_ab` (a1–a6 of 7 tests; `evidence/localnet/S-A.run.txt`) | Settlement as state-write + chained token legs is safe: a failing last leg reverted the state write and the token transfer (`ProgramExecutionFailed("Guest panicked: Internal asked to fail")`, all balances unchanged). Self re-entry works and a top-level call into the internal entrypoint is rejected, so `repay_flash` can be an internal self-call gated on `caller_program_id`. 10 chained calls (11 executions) accepted, 11 chained rejected (`MaxChainedCallsDepthExceeded`). Not done in M0: a sequencer-layer dropped-tx observation, a foreign-program debit attempt, and the explicit `InconsistentAccountPreState` negative (A7). |
| S-B custody rule | **GO** (v0.2.4 builtin token) | same runs (b1–b4; `evidence/localnet/S-B.run.txt` is from the 2026-10-16 re-run) | Vault = token-owned PDA claimed via chained `InitializeAccount` under Argo's seed; anyone credits it, only Argo debits it by seed. Wrong seed → `InvalidAccountAuthorization` (test a4, recorded in `evidence/localnet/S-A.run.txt`; the same runtime check a foreign program would hit; a foreign-program guest was not built); a user-signed debit the vault did not authorise → `Sender authorization is missing` (the builtin token's sender check); credit before claim → `ClaimedUnauthorizedAccount` and a later `Init` still succeeds, so `create_market` claims both vaults first. M1 targets the v0.2.4 builtin token behind a `token_core` adapter. M0 did not establish which token program the public testnet runs: the former b5 check read `getProgramIds`, which v0.2.4 hardcodes, so it was withdrawn. On v0.2.4 the sequencer's genesis registers `clock` and the sequencer updates `CLOCK_01` every block, so the spec proposes `CLOCK_01` as the accrual clock for M1 (§5, pending rekt's review and a live check on the relaunched 0.2 testnet). |
| S-C event surface | **NO-GO** (as expected) | hetzner2 2026-09-02 + CI, `spikes/S-C/run.sh` | v0.2.4 `ProgramOutput` and `Block` have no event field (source), and `getTransaction` returns only `(the submitted LeeTransaction, block id)`, so no execution output of any kind reaches a client (`evidence/localnet/S-C-*.json`). Reference liquidator/monitor observe by PDA polling: 8.4–8.8 ms per `getAccount` on localhost (200 calls to one account; no multi-account sweep was measured, that is M4 work). Events landed on LEZ `dev` after v0.2.4 (PR 705/707, indexer `getEvents` in PR 713) and ship in v0.2.5-rc1 and v0.3.0, not in v0.2.4; the observation layer sits behind a trait so M4 swaps to events on a version that has them. |
| S-D compute budget | **GO** | hetzner2 2026-09-02 + CI, `spikes/S-D/run.sh` (`evidence/localnet/S-D.run.txt`) | Per-execution cycles (po2-padded, the runtime's own metric; 10k stress iterations trip `Session limit exceeded` as asserted): Argo supply-shaped leg 239k, builtin token Transfer 131k, Argo leg emitting one transfer + one self-call 271k, self-call leg 74k; that three-leg chain totals 476k. Arithmetic costs ~5.3k cycles per accrual/health/LIF-shaped iteration (bitwise 256/128 division), so a liquidation extrapolated as PayOut leg + 40 iterations + one more transfer is ~0.6M, under 2% of the 32M budget; M3 measures the real thing and M1 uses a cheaper widening divide. **Account size dominates**: carrying a state account through one execution costs 544k cycles at 1 KB, 4.8M at 10 KB, 23.7M at 50 KB (70.6% of the 32M = 33,554,432-cycle limit), consistent with lez-programs' benchmark of ~33M extra cycles for a 100 KB owned account. Every Argo account stays under 1 KB. |
| S-E fee within deshield | **PARTIAL** (init step established for the stock v0.2.4 wallet deshield; linkability not evaluated) | hetzner2 2026-09-02 + CI, `spikes/S-E/run.sh`, wallet v0.2.4 vs local sequencer | v0.2.4 has no transaction fees, so "fund the fee inside the deshield" is vacuous on v0.2.4. The stock v0.2.4 wallet deshield into a never-initialised public account is rejected by the privacy circuit while proving (`Cannot claim unauthorized account`, saved in `evidence/localnet/S-E-e0b.wallet.txt`), so with that wallet path the SDK first initialises the ephemeral account with its own key (E0, public tx), then deshield (E1: A=300) → a public op signed by A (E2: native transfer A→B in M0) → reshield to a fresh private note (E3: A=0). Each step's wallet output and raw `getTransaction` are under `evidence/localnet/S-E-*`, and the script exits non-zero if any step deviates. E4 (which accounts co-appear with A in public block data) was NOT evaluated: raw blocks are saved but need a borsh decoder (M5). E5 notes (added 2026-10-16) record how A's init could be paid once fees exist (a conditional analysis; the routes are untested) and an untested co-signed deshield that may avoid E0. Upstream fees (PR 801) are in v0.2.5-rc1 and v0.3.0, not v0.2.4; S-E is re-run on the version Argo ships on. |

## Corrections (2026-10-16)

- S-B b5 withdrawn. It compared the builtin token id with the public
  testnet's `getProgramIds` reply, which v0.2.4 builds from ids compiled into
  the sequencer (`lez/sequencer/service/src/service.rs:199-214`, `// TODO: Get
  programs from state`), so it proved nothing about the chain. The test is
  removed; details in `docs/m0/spikes/S-B-lp0013-custody.md`.
- "No clock on the testnet" withdrawn for the same reason. v0.2.4 genesis
  registers the clock program and the sequencer updates `CLOCK_01` every
  block; M0 did not read `CLOCK_01` on the public testnet.
- Also withdrawn with b5: "the testnet runs the builtin token" (S-B row and
  the `Cargo.toml`, `spike_ab.rs` and `spikes/S-B/run.sh` comments) and
  "LP-0013's authority token is not on the testnet" (report item 2 as
  submitted).
- Upstream PR statuses updated. PR 801 (fees) and PR 817 (claims and balance
  decrease) were open when the spikes were written on 2026-09-02; they merged
  to `dev` on 2026-09-03 and 2026-09-04. The events PRs had already reached
  `dev` on 2026-08-27, when PR 705 merged (707 and 713 through its stacked
  branches), but on 2026-09-02 no tag shipped them. All of them are in
  v0.2.5-rc1 and v0.3.0, not in v0.2.4.
- S-E: E0b is scoped to the stock wallet deshield, and E5 notes are added.
- S-D: the 50 KB figure is 70.6% of the per-execution limit (it said 74%).
