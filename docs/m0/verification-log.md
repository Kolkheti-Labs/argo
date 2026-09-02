# M0 verification log

Every entry names the host, the commit, the command, and the outcome. A
"clean host" entry means a fresh clone on a machine that had never built
Argo; those are the ones the acceptance gate counts. Development-box entries
are recorded too, so a reviewer can see what was tried where.

## 2026-09-02 — hetzner2 (development box, not clean)

Host: Hetzner CX43, 8 vCPU / 15 GB, Ubuntu, x86_64. Toolchain: rust 1.94.0
(rustup), risc0 rust 1.94.1 + r0vm 3.0.5 + cargo-risczero 3.0.5 (rzup),
`libclang-dev`, `libpcsclite-dev` present. Tree: rsync of the working copy
(uncommitted), not a git clone. Env: `/mnt/HC_Volume_105321168/argo-env.sh`.

| Step | Command | Outcome |
| --- | --- | --- |
| pure crates | `cargo test -p argo_core -p irm_core` | 7 passed (5 + 2), also green on macOS arm64 |
| host crates | `cargo check -p spike_vault_program -p argo_lending_program ...` (macOS) | clean after adding `borsh` dep |
| guest build (D4) | `cargo build --release -p harness_runner` (builds both `*_methods` via `risc0_build::embed_methods`) | OK. `argo_lending` ELF 183,388 B, `spike_vault` ELF 208,868 B |
| S-A/S-B tests | `cargo test --release -p spike_integration_tests --test spike_ab` | 7 passed, 0 failed, 5.89 s (`RISC0_DEV_MODE=1`, real guest ELFs, builtin token program) |

Image ids from this build (they change with any guest source or profile
change, so they are recorded, not relied on):

```
ARGO_LENDING_ID = [2758225782, 2927683024, 2803086246, 1614634390, 326454054, 3248717555, 1139695692, 1910660946]
SPIKE_VAULT_ID  = [3060177745, 3358717213, 3856548115, 2817312362, 2773267506, 1305010604, 221952450, 289626746]
```

Defects found only by building, not by reading:

1. `RISC0_HOME` pointed at the wrong directory in the first env file; risc0-build
   reports "Risc Zero Rust toolchain not found" rather than the path it tried.
2. SPEL's `#[lez_program]` rewrites the two-segment path `SpelOutput::execute`
   into `execute_with_claims`, which does not accept `Vec<AccountPostState>`.
   Use the fully qualified `spel_framework::SpelOutput::execute`.
3. `spike_vault_program` and `spike_integration_tests` were missing `borsh` /
   `serde` in their own manifests (the guest crate had them, the host crates
   did not).

| harness smoke (D3) | `./harness/localnet.sh smoke` | GREEN 2026-09-02T18:38Z: sequencer boot → `argo_deploy argo_lending` (tx e1dabf1b…) → `argo_initialize` (tx af03b694…) → `getAccount` shows Config PDA `GDXzcxkq…` owned by `ARGO_LENDING_ID` |

| S-D | `spikes/S-D/run.sh` | GO, cycle table in `docs/m0/verdicts.md` |
| S-C | `spikes/S-C/run.sh` | NO-GO (no event surface), 8.8 ms/getAccount local |
| S-E | `spikes/S-E/run.sh` | E0b rejected at prover; E0 init tx c061835f… block 29; E1 deshield 872749a0… block 56; E2 A→B 40da2bcb… block 109; E3 reshield f8f89f79… block 136; A final balance 0 |

Executor errors observed (quoted from the run, they are the S-A/S-B evidence):

```
A2  ProgramExecutionFailed("Guest panicked: Internal asked to fail")   -> no state applied
A4  InvalidProgramBehavior(InvalidAccountAuthorization { account_id: CWU2Cn... })
A5  ProgramExecutionFailed("Guest panicked: Sender authorization is missing")
A6  MaxChainedCallsDepthExceeded                                      (11 calls; 10 accepted)
B2  InvalidProgramBehavior(ClaimedUnauthorizedAccount { account_id: CWU2Cn... })
```

## Clean-host run

(pending: fresh clone on a host that has never built Argo, following README only)
