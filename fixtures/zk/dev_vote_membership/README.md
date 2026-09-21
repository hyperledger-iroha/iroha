# Development-only vote-membership artifacts

This directory intentionally contains no generated proof or key. The sole generator
is `cargo run -p xtask --bin xtask --features dev-tools,dev-vote-fixture -- zk-dev-vote-fixture`.
It produces `dev_vote_membership_{meta.json,proof.zk1,vk.zk1}` and requires both raw
IPA verification and production rejection before writing. Outputs cannot register
an election key. See `specs/governance_vote_tally.md` for the exact relation and
candidate-specific validation requirements.
