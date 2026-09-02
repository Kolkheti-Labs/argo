# Lessons (append-only; rule + why + how to apply)

- **Never rsync `--delete` into a box while a run is writing there.** Why: an S-E run
  lost its `evidence/localnet/` mid-flight when a code push deleted the directory the
  spike was writing into; every tx had landed but no artifact survived. Apply: push code
  with `--exclude evidence/`, pull evidence back with a separate rsync, and never push
  while a spike or harness job is in flight on the same tree.
- **`pkill -f <pattern>` from an ssh one-liner kills the ssh session itself** when the
  pattern appears in the remote command line. Apply: kill by pid file, or run pkill from
  a script file whose command line does not contain the pattern.
- **A build is not verified until it runs from an empty toolchain home.** Why: the first
  clean run failed in 4 s on a bundle without HEAD and on rzup ignoring `RISC0_HOME`,
  after every developer-box build had passed. Apply: `harness/clean-host.sh` before any
  "done" claim; keep it in the repo.
- **SPEL's `#[lez_program]` rewrites `SpelOutput::execute(...)` only when spelled with a
  two-segment path.** Apply: write `spel_framework::SpelOutput::execute(posts, calls)`.
