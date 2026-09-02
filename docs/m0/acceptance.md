# M0 acceptance gate

M0 is "done" when every row below has an artifact in this repository and a
verification log produced on a clean host from a fresh clone. The reviewer
should be able to reproduce each row without talking to us.

| # | Deliverable (proposal wording) | Artifact | Verified by |
| --- | --- | --- | --- |
| D1 | Repo, dual licence MIT + Apache-2.0, CI | `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `.github/workflows/argo-ci.yml` | CI green on `main` for the M0 tag; licence headers checked by `cargo deny` |
| D2 | SPEL scaffolding | `argo_core/`, `irm_core/`, `methods/guest/` compile as a SPEL program with one placeholder instruction; IDL generated | `cargo build -p methods` on the clean host; IDL file committed under `idl/` |
| D3 | Standalone-sequencer test harness | `harness/localnet.sh`, `harness/lib.sh` (RPC probes), `harness/capture.sh` | `./harness/localnet.sh smoke` exits 0 on the clean host and in CI; boots sequencer, funds a wallet, deploys the placeholder program, reads it back over JSON-RPC |
| D4 | Resolved `ring`/riscv32 build | `docs/m0/riscv32-build.md` + the Cargo patch/feature lines it names | guest builds for `riscv32im-risc0-zkvm-elf` on the clean host with no manual steps beyond the documented apt packages |
| D5 | Account-sharding + state-layout spec | `docs/spec/state-layout.md` | Reviewed by rekt and adp; every instruction lists its touched-account set and that set is independent of market/position count |
| D6 | Go/no-go spikes ×5 | `docs/m0/spikes/S-{A..E}.md`, code under `spikes/`, `docs/m0/verdicts.md` | Each spike's experiment reruns from `spikes/run.sh S-X` on the clean host; verdict file states go/no-go and, for no-go, the M1–M3 change |

## Clean-host rule

Reading code does not find environment defects. Every row is verified by
running it from a fresh `git clone` on a host that has never built Argo,
following only `README.md`. The host, commit hash, and full log are recorded in
`docs/m0/verification-log.md`.

## Evidence rule

The LEZ testnet is wiped and redeployed periodically, which orphans program ids
and transaction hashes. Any claim that references chain state must be backed by
a raw JSON-RPC response committed under `evidence/`, captured by
`harness/capture.sh`, and regenerable by rerunning that script. M0 spikes run
on the local standalone sequencer, so this rule mostly matters from M1 on, but
the tooling ships in M0.

## Out of scope for M0

Any lending logic beyond a placeholder instruction, any testnet deployment,
any GUI/CLI code. Those start in M1 after the verdicts are accepted.
