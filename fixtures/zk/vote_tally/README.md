# Vote Tally Fixtures

This directory is intentionally empty. Regenerate the governance vote tally verifying key and proof bundles with:

```
cargo xtask zk-vote-tally-bundle --out fixtures/zk/vote_tally --print-hashes --summary-json summary.json --attestation bundle.attestation.json
```

The command runs the deterministic generator embedded in `xtask/src/vote_tally.rs`, producing:

- `vote_tally_vk.zk1`
- `vote_tally_proof.zk1`
- `vote_tally_meta.json`
- Optional `bundle.attestation.json` capturing the summary plus Blake2b-256 digests of each artifact. The generator is fully deterministic, so re-running the command produces identical artefacts (including the proof envelope).
- Use `--summary-json -` to emit the summary JSON to stdout or provide a path to write a file alongside the artifacts.

Use `--verify` to compare freshly generated artefacts with the checked-in versions (including `bundle.attestation.json`, which guards metadata, file lengths, and proof digest). This flag assumes the fixture directory already contains a baseline bundle.

First run the command without `--verify` to seed the directory, then rerun with `--verify` to confirm no local drift.
