# Argo M0 — Foundations & de-risking spikes

Milestone: M0, 2 weeks, $5,000, invoiced on the "done" gate in `docs/m0/acceptance.md`.
Source of truth for deliverables: accepted proposal, milestone table
(logos-co/rfp issue 141, mirror at ~/Desktop/rfp008-proposal.md) and the M0
tracker https://github.com/logos-co/rfp/issues/170 (done gate includes spec
reviewed WITH LOGOS and CI green against the harness).

Gate 0 (before any paid engineering): contract signed. Confirmed by Davit: 2026-09-02.

## Deliverables (verbatim from the proposal)

- [ ] D1 Repo, dual licence MIT + Apache-2.0, CI
- [ ] D2 SPEL scaffolding
- [ ] D3 Standalone-sequencer test harness
- [ ] D4 Resolved `ring`/riscv32 build
- [ ] D5 Account-sharding + state-layout spec
- [ ] D6 Go/no-go spikes, each with a written verdict:
  - [ ] S-A chained-call re-entry + ChainedCall revert/atomicity
  - [ ] S-B LP-0013 custody rule
  - [ ] S-C LP-0012 event surface
  - [ ] S-D per-tx compute budget
  - [ ] S-E fee-funding-within-deshield

## Tasks

### Week 1

- [x] T0 Read handoff, proposal, RFP spec, LP-0002 notes; pull LP-0002 demo script + CI as reference
- [x] T1 Repo skeleton: layout per proposal §1, LICENSE-MIT, LICENSE-APACHE, NOTICE, .gitignore (D1 part)
- [x] T2 Spike plan written and reviewed before code: `docs/m0/spike-plan.md` + one file per spike under `docs/m0/spikes/`
- [x] T3 Cargo workspace laid out like logos-blockchain/lez-programs (not `spel init`): `argo_core`, `irm_core` (no_std), `methods` + `methods/guest` (own workspace, committed Cargo.lock), `spikes/*`; SPEL pinned at rev 1ef0500f, LEZ at tag v0.2.4, risc0 =3.0.5; `rust-toolchain.toml` 1.94.0
- [x] T4 `ring`/riscv32: fixed upstream; guests build from a fresh toolchain (clean run) and documented in docs/m0/riscv32-build.md in LEZ v0.2.4 (`risc0-zkvm default-features=false, features=["std"]`). D4 = confirm the guest builds from a clean host, record remaining pins (`ruint =1.17.0`, `enum-ordinalize 4.3.2`, `getrandom_backend="custom"`, macOS CFLAGS) in `docs/m0/riscv32-build.md`
- [x] T5 Harness SMOKE GREEN on hetzner2 2026-09-02 (boot→deploy→initialize→RPC read-back): `harness/localnet.sh` boots a standalone sequencer on a scratch dir, bootstraps a wallet, exposes JSON-RPC probe helpers; honours `CARGO_TARGET_DIR`; no `ss`/`pkill` dependency (D3)
- [~] T6 CI (workflows written, never run — needs a GitHub push, gated on Davit): ci-image workflow (content-hashed GHCR image, LP-0002 shape), `argo-ci.yml` = fmt + clippy + unit + harness e2e smoke at `RISC0_DEV_MODE=1` (D1)
- [x] T7 Evidence model (spike runners snapshot raw getTransaction/getProgramIds into evidence/; harness/capture.sh folded into the spike scripts): `harness/capture.sh` snapshots raw JSON-RPC responses per claim into `evidence/`, regenerable by one command (testnet wipes orphan ids)

### Week 2

- [x] T8 Spike S-A: GO, 7/7 in-process on hetzner2 (sequencer dropped-tx observation still to record in harness): negative tests (failed leg reverts earlier state writes; re-entry into caller; wrong seed; forged vault debit)
- [x] T9 Spike S-B: GO on builtin token, B5 captured (testnet = 5 programs, no clock): program-owned vault credited by anyone, debited only under PDA authorisation; fresh-PDA claim-before-fund confirmed on v0.2.4
- [x] T10 Spike S-C: NO-GO → PDA polling (verdicts.md): event/log surface probed on v0.2.4; verdict = events vs PDA polling for liquidator/monitor
- [x] T11 Spike S-D: GO, cycle table in verdicts.md (account size is the cost driver): measured cycles for a supply-shaped and a liquidate-shaped guest; compare to any runtime limit; note the >6 GB RAM outer-prover fact
- [x] T12 Spike S-E: GO w/ init-step design change; fee N/A on v0.2.4: v0.2.4 has no fees, so the live question is the ephemeral-account init rule (deshield to a never-initialised public account is rejected) and linkability; record the fee design for PR 801's model
- [~] T13 State-layout spec drafted with v0.2.4 facts; awaiting rekt/adp review, then Logos review on issue 170: `docs/spec/state-layout.md`: PDA seeds, account byte layouts, touched-set per instruction, sharding argument for P2 (D5)
- [x] T14 Verdicts (all five written): `docs/m0/verdicts.md`, one go/no-go per spike; any no-go carries the M1–M3 design change
- [x] T15a Fresh-toolchain + fresh-clone run GREEN on hetzner2 volume 2026-09-02 (27 min); [ ] T15b same script on a never-used box (needs a box from Davit) (hetzner2 volume or a fresh box): `git clone && ./harness/localnet.sh && cargo test` green from nothing. Record host, commit, log
- [x] T16 adversarial-verify pass 2026-09-02: 3 findings, all fixed and re-run
- [~] T17 Milestone report drafted (docs/m0/report.md); final after CI + fresh-box run: link every D1–D6 to its artifact and its verification log

## Decisions surfaced by the research (need a call before M1)

- Token program: v0.2.4 builtin (no authorities) vs lez-programs' authority token (Borsh-incompatible). S-B B5 checks which the testnet runs; keep `token_core` behind one adapter module either way.
- Observation layer: events exist only on `dev` (post-v0.2.4). M4 ships PDA polling; event follower behind the same trait.
- Ephemeral account: SDK must init the account with its own key before the deshield (E0). Extra public tx per private op.
- Compute: design liquidation for BOTH per-leg 32M (v0.2.4) and chain-total budget (fee PR 801).

## Blocked / needs Davit

- Contract signature confirmation (Gate 0)
- Any push to GitHub (standing rule: none without explicit approval)

## Review (2026-09-02, end of day 1)

Done and verified: D2, D3, D4, D6 (all five verdicts, reproduced from a fresh toolchain). Drafted: D5 (needs human review), D1 (licences done; CI never executed because nothing is pushed). Not done: fresh-box run, GitHub repo + CI green, spec review, final report. Six local commits, nothing pushed.
