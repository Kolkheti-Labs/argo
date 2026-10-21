# evidence/testnet

`getProgramIds-20260902T181548Z.json` is the public LEZ testnet's reply to
`getProgramIds` on 2026-09-02, saved by `harness/probe-testnet-token.sh`.

Superseded on 2026-10-16. On LEZ v0.2.4 that RPC returns a map of five ids
compiled into the sequencer, four builtin programs and the privacy-preserving
circuit (`lez/sequencer/service/src/service.rs:199-214`, `// TODO: Get
programs from state`), not the chain's registered programs. So this file does not show
which programs the testnet ran, and the absence of `clock`, `ata` and `vault`
from it means nothing. The S-B check that used it (b5) was withdrawn; see
`docs/m0/spikes/S-B-lp0013-custody.md`. Logos reported a testnet reset on
Discord on 2026-10-10, so this capture cannot be re-taken against the chain it
came from. The file is kept for the record because the withdrawn M0 claims
cited it; it does not back any current claim.
