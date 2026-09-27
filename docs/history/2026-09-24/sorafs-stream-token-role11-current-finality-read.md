# SoraFS role-11 StreamToken current custody block finality

The role-11 native custody reader now has a same-State current-block read that pairs the
provider-scoped retained control row with the exact block's durable Kura revision-4
finality. It rejects a stale height, foreign binding, corrupt custody history, missing
CommitQC, forked certificate, and an attempt to reuse an older block's certificate.
It returns `None` for an unconfigured provider only after the current block itself
has authenticated finality. The returned capability has private fields and no
operation, Check, key-use, or token-release method.

The source is [`stream_token_custody.rs`](../../../crates/iroha_core/src/query/stream_token_custody.rs)
and the scoped fixture is
[`reader_tests.rs`](../../../crates/iroha_core/src/smartcontracts/isi/sorafs_stream_token_custody/reader_tests.rs).
The fixture uses an authenticated software custody binding and an actual four-validator
test finality artifact; it does not supply production signing authority. The role-11
custody error text and rotation fixtures now describe and exercise software handles;
signed enrollment, key-generation checks and revocation rules are unchanged. The role-11
Reserve/terminal replay remains in
[`historical_execution.rs`](../../../crates/iroha_core/src/query/stream_token_authority/historical_execution.rs),
while the native role-11 Check still returns `CheckUnavailable` in
[`sorafs_stream_token_authority.rs`](../../../crates/iroha_core/src/smartcontracts/isi/sorafs_stream_token_authority.rs).
Therefore the end-to-end production `SignerOperationStateSourceV1` adapter, native
Check submission/readback, independently pinned current custody and time authority,
and stream-token issuance qualification remain open.

Validation: `scripts/cargo_fast.sh --stable-local-metadata --incremental -- test
-p iroha_core --lib current_stream_token_custody_requires_same_state_finality_and_grants_no_operation
-- --nocapture` passed (1/1). The final test-only receipt binding also passed
in the fresh merged Core test binary alongside the role-13 selector. The
production code and assertions were unchanged. `rustfmt --edition 2024` on the four touched source files and
`git diff --check` passed.
