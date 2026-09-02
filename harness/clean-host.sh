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
# The rzup bootstrap script ignores RISC0_HOME and always writes $HOME/.risc0/bin/rzup,
# so run it with HOME pointed at the workdir and move the binary. rzup itself honours
# RISC0_HOME for everything it installs afterwards.
if [ ! -x "$RISC0_HOME/bin/rzup" ]; then
  mkdir -p "$RISC0_HOME/bin"
  HOME="$W" bash -c 'curl -sL https://risczero.com/install | bash' >/dev/null
  mv "$W/.risc0/bin/rzup" "$RISC0_HOME/bin/rzup"
fi
# Pinned to the set the CI image uses. No `|| true`: a missing component must fail here.
rzup install rust 1.94.1
rzup install cpp 2024.1.5
rzup install r0vm 3.0.5
rzup install cargo-risczero 3.0.5
echo "== fresh clone =="
rm -rf argo; git clone -q -b main "$SRC" argo; cd argo
# rust-toolchain.toml selects the host toolchain; the first cargo call installs it.
cargo --version
rustup show active-toolchain
git rev-parse HEAD
echo "== README steps =="
cargo test -p argo_core -p irm_core
./harness/localnet.sh smoke
./spikes/run.sh all
echo "== CLEAN-HOST GREEN $(date -u +%FT%TZ) @ $(git rev-parse --short HEAD) =="
