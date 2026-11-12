# lez-programs v2.0.1 programs (vendored binaries)

Compiled guests from logos-blockchain/lez-programs tag `v2.0.1` (commit b17f919), MIT licensed
(Copyright (c) 2026 Logos Blockchain; see `NOTICE` at the repo root). They are the builds deployed on
testnet-legacy, per `DEPLOYMENTS.md` on lez-programs `main` ("Programs built from tag v2.0.1 with
cargo risczero build"; that file is not in the v2.0.1 tag itself):

| File | Image id (program id) | sha256 |
| --- | --- | --- |
| `token.bin` | `14b3bc6cc129d0359f9595ba0f60a13e4a8df08ffe8eb1a83251c47abecc67a8` | `bc8c24bddefc00628a7f659fe2d04dd074868250167a0334b0e6ae5c13e56bc9` |
| `twap_oracle.bin` | `5681e27c27b3ece4a7c161bbf8e12e6ece6ca69925a87b77f9e193fa4a249b31` | `53c1d995d72c42226fc41a8bbd8a042667ed91a0add0db4c18a68468db809dd5` |

Argo pins the token id (`docs/m1/decisions.md` D1), so tests must deploy exactly this binary; a local rebuild
outside the Docker builder gets a different image id. `twap_oracle.bin` is used only by the sequencer e2e
(`harness/runner/src/bin/argo_m1_e2e.rs`) to create a real price account. Tests assert both image ids.

Rebuild (needs Docker; done on hetzner2, 2026-11-09):

    git clone https://github.com/logos-blockchain/lez-programs && cd lez-programs
    git checkout v2.0.1
    RISC0_DOCKER_BUILD_NETWORK=host ./scripts/build-guests.sh
    # -> target/guest/token.bin, target/guest/twap_oracle.bin
