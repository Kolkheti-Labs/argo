#!/usr/bin/env bash
# Clean-environment verification: fresh clone, fresh Rust + risc0 toolchains in
# throwaway homes, then the README steps. Catches hardcoded paths and
# undocumented dependencies that a developer box hides.
#
#   ./harness/clean-host.sh <git-url-or-bundle> <workdir>
#
# System packages are NOT installed here (needs root); the README lists them.
set -euo pipefail
SRC=${1:?git url or bundle}; W=${2:?workdir}
mkdir -p "$W"; cd "$W"
export RUSTUP_HOME="$W/rustup" CARGO_HOME="$W/cargo" RISC0_HOME="$W/risc0"
export PATH="$CARGO_HOME/bin:$RISC0_HOME/bin:$PATH"
export CARGO_TARGET_DIR="$W/target" TMPDIR="$W/tmp"; mkdir -p "$TMPDIR"
export RISC0_DEV_MODE=1
echo "== toolchains into $W (rust from rust-toolchain.toml, risc0 3.0.5) =="
[ -x "$CARGO_HOME/bin/cargo" ] || curl -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --default-toolchain none >/dev/null
[ -x "$RISC0_HOME/bin/rzup" ] || { curl -sL https://risczero.com/install | RISC0_HOME="$RISC0_HOME" bash >/dev/null; }
rzup install r0vm 3.0.5 >/dev/null 2>&1 || true
rzup install cargo-risczero 3.0.5 >/dev/null 2>&1 || true
rzup install rust >/dev/null 2>&1 || true
rzup install cpp >/dev/null 2>&1 || true
echo "== fresh clone =="
rm -rf argo; git clone -q "$SRC" argo; cd argo
rustup show active-toolchain
git rev-parse HEAD
echo "== README steps =="
cargo test -p argo_core -p irm_core
./harness/localnet.sh smoke
./spikes/run.sh all
echo "== CLEAN-HOST GREEN $(date -u +%FT%TZ) @ $(git rev-parse --short HEAD) =="
