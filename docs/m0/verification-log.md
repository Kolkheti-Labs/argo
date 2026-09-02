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
| S-A/S-B tests | `cargo test --release -p spike_integration_tests --test spike_ab` | 8 passed (a1–a6, b2, b5), 0 failed (`RISC0_DEV_MODE=1`, real guest ELFs, builtin token program) |

Image ids from this build on the dev box (they change with any guest source,
lockfile or profile change; the guest lockfiles are committed and CI builds
with `RISC0_BUILD_LOCKED=1` so the ids are a function of the tree; the Docker
`cargo risczero build` path was not run in M0):

```
ARGO_LENDING_ID = [1155194347, 1601915833, 802455914, 395287946, 3013651780, 1058916772, 1694453169, 1300607297]
SPIKE_VAULT_ID  = [4107046892, 3062661010, 1329261408, 143454969, 3861191297, 2985587711, 1721357874, 101925694]
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
| S-E | `spikes/S-E/run.sh` (port 3049) | E0b rejected at prover (`Cannot claim unauthorized account`, `evidence/localnet/S-E-e0b.wallet.txt`); E0 init tx 493bf112… block 29; E1 deshield 6aca60f7… block 56 (A=300); E2 init B 54cebd8c… block 82, A→B b6c19015… block 109; E3 reshield f3ff12d1… block 136; A final balance 0; verdict PARTIAL (E4 not evaluated). Raw payloads: `evidence/localnet/S-E-*` |

Executor errors observed (quoted from the run, they are the S-A/S-B evidence):

```
A2  ProgramExecutionFailed("Guest panicked: Internal asked to fail")   -> no state applied
A4  InvalidProgramBehavior(InvalidAccountAuthorization { account_id: CWU2Cn... })
A5  ProgramExecutionFailed("Guest panicked: Sender authorization is missing")
A6  MaxChainedCallsDepthExceeded                                      (11 calls; 10 accepted)
B2  InvalidProgramBehavior(ClaimedUnauthorizedAccount { account_id: CWU2Cn... })
```

4. First clean-environment attempt (fresh toolchain homes + fresh clone on
   hetzner2's volume) failed in 4 s: the git bundle carried no HEAD, so
   `git clone` checked out nothing; and the rzup bootstrap ignores
   `RISC0_HOME`, writing to `$HOME/.risc0` (the CI Dockerfile works around
   the same thing). `harness/clean-host.sh` now clones `-b main`, runs the
   bootstrap with `HOME` redirected, and no longer swallows `rzup install`
   failures.

## Fix round after the adversarial review (2026-09-03, hetzner2)

A 25-agent adversarial review of the first submission confirmed 15 findings
(stale root `Cargo.lock`, no guest lockfiles, spike runners that exited 0 on
failure, S-E evidence ignored by `*.log`, docs describing experiments wider
than what ran, `Initialize` with an unauthenticated admin, clean-host script
cloning the bare `main`). After the fixes, on hetzner2 with
`RISC0_BUILD_LOCKED=1`: `cargo test --locked` 16 passed; clippy `--locked`
clean; `./harness/localnet.sh smoke` green with `Config.admin` equal to the
funder that signed `Initialize`; `./spikes/run.sh all` exit 0 with verdicts
S-A GO, S-B GO, S-D GO (asserted), S-C NO-GO, S-E PARTIAL; 26 evidence files
regenerated. The image ids above are from this build.

## CI (GitHub Actions, ubuntu-latest, container from `.github/docker/ci.Dockerfile`)

**GREEN 2026-09-02, PR #1, run https://github.com/Kolkheti-Labs/argo/actions/runs/33674571127**: `ci-image` (10 min), `lint` (fmt +
clippy under the lez-programs lint set), `unit` (pure crates + both riscv32
guests + IDL drift check), `e2e-localnet` (sequencer and wallet installed from
LEZ v0.2.4 source, harness smoke, then all five spike runners). This is the
first run on a machine that had never built Argo, so it also stands as the
clean-host evidence for the harness and spikes.

## Clean-environment run (interim, hetzner2 volume)

Fresh `RUSTUP_HOME`/`CARGO_HOME`/`RISC0_HOME`/target under
`/mnt/HC_Volume_105321168/argo-clean`, fresh clone from a bundle, README
steps via `harness/clean-host.sh`. Same kernel and apt packages as the dev
box, so this is not the final clean-host run; that needs a box that never
built Argo.

**GREEN, 2026-09-02T19:18:46Z** (`harness/clean-host.sh`, 27 min wall clock
from empty toolchain homes on 8 vCPU). The run was made from a bundle of a
pre-rebase commit whose tree is identical to `522ddda` on the `m0` branch
(the original SHA `e06df05` no longer exists after the branch was rebased
onto the base commit; `git diff e06df05 522ddda` was empty at the time).
Steps and outcomes:

| Step | Outcome |
| --- | --- |
| rustup (1.94.0 via `rust-toolchain.toml`), rzup rust 1.94.1 / cpp 2024.1.5 / r0vm 3.0.5 / cargo-risczero 3.0.5 | installed into `argo-clean/{rustup,cargo,risc0}` |
| `git clone <bundle>` | tree == `522ddda` |
| `cargo test -p argo_core -p irm_core` | 7 passed |
| `./harness/localnet.sh smoke` | sequencer + wallet built from source, `argo_lending` deployed as `d9b7db26…`, Config PDA initialised and read back: GREEN |
| `spikes/run.sh all` | S-A GO (6 tests), S-B GO (4 tests incl. b5 token-id match), S-D GO (heaviest leg 271,395 cycles; 50 KB account 23.68M), S-C NO-GO (8.44 ms/getAccount), S-E landed end to end (E0b rejected at prover) |

Cycle counts differ from the dev-box run by <0.5% (different guest image
from a different toolchain home), which is the expected noise.

Still not the final clean host: same kernel and apt packages as the dev box.
The acceptance gate's last row needs one run on a box that never built Argo.
