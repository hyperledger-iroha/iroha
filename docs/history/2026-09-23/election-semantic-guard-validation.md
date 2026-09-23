# Standalone election semantic guards

The current `optimizations` Core source keeps standalone private elections
closed while the required committee-free, dropout-resilient anonymous ballot
and sound tally protocol is unresolved. Changing voting roles does not open the
standalone ZK route, retired Halo2 ballot/tally verifier-key circuits remain
rejected, and plain-ballot resource and conviction limits remain fail-closed.

After the current Core rebuild, these focused commands each passed one test:

- `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_core --lib standalone_zk_semantic_guard_stays_closed_independently_of_vote_roles -- --nocapture`
- `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_core --lib resolve_ballot_and_tally_vk_reject_retired_halo2_vote_circuits -- --nocapture`
- `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p iroha_core --lib plain_ballot_resource_and_conviction_limits_fail_closed -- --nocapture`

These are negative admission controls. They do not supply credential-linked
ballots, confidential bonds, hidden-choice-preserving updates, a closed-corpus
tally proof, or late-dropout completion. F11 and final release remain open.
