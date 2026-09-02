# Guest build for riscv32im-risc0-zkvm-elf (D4)

Status: draft from source reading, to be confirmed by a clean-host build.

## The `ring`/riscv32 problem, as of LEZ v0.2.4

The cross-compile failure the proposal anticipated (`ring` pulled into the
guest by a default feature of `risc0-zkvm`) is fixed upstream. LEZ's
workspace pins

```toml
risc0-zkvm = { version = "3.0.5", default-features = false, features = ["std"] }
```

and SPEL v0.6.0 closed its issue 165 on the same fix. Argo's guest crates use
`risc0-zkvm = { version = "=3.0.5", default-features = false }` and inherit
the fix. No `[patch]` section is needed.

## Pins that still matter

| Pin | Where | Why |
| --- | --- | --- |
| `ruint = "=1.17.0"` | guest `Cargo.toml` | ruint 1.18 raised MSRV to rustc 1.90; the risc0 guest toolchain ships 1.88 |
| `CFLAGS_riscv32im_risc0_zkvm_elf = "-march=rv32im -nostdlib"` | `.cargo/config.toml` | cc-rs injects macOS flags into the RISC-V cross compiler otherwise |
| `[profile.release] debug = 0, strip = "symbols"` | guest `Cargo.toml` | part of the image id; changing it changes the program id and every PDA |
| `risc0-build = "=3.0.5"`, r0vm 3.0.5, cargo-risczero 3.0.5 | methods crates, CI image | image id reproducibility |

## Two ways to build

1. `cargo build -p <x>_methods` runs `risc0_build::embed_methods()`, which
   builds the guest with the rzup-installed guest toolchain and embeds
   `<X>_ELF` / `<X>_ID`. This is what the harness and CI use; no Docker.
2. `cargo risczero build --manifest-path <guest>/Cargo.toml` builds inside
   the risc0 Docker builder for a reproducible image id. This is the path for
   the deployed artifact and for the `check-idl`-style reproducibility gate
   from M1 on.

Only path 1 was run in M0 (its image ids are in the verification log). Path 2
is the deployment path from M1 on, where its id will be compared against
path 1's. Both guest workspaces commit their `Cargo.lock` and CI sets
`RISC0_BUILD_LOCKED=1`, so path 1's image id is a function of the committed
tree; `enum-ordinalize` resolves to 4.4.2 there and builds fine on the 1.94.1
guest toolchain (the 4.3.2 pin `spel init` applies was not needed).

## Verification

See `docs/m0/verification-log.md`: both guests built on hetzner2 (x86_64) via `risc0_build::embed_methods` on 2026-09-02; the clean-environment run repeats it from a fresh toolchain.
