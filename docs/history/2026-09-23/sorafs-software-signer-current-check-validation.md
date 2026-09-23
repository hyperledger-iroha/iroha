# SoraFS software signer and Current Check validation

The current `optimizations` daemon can load an owner-only software credential
for an explicitly supplied operation state source. Its coordinator refuses an
absent source before credential I/O. The non-signing role-14 Current Check
runtime retains the original signed envelope through ambiguous submission and
requires successful native application, same-State custody/audit, State/Kura
finality, qualified time, and a retained floor before returning an observation.

Focused commands on the current checkout:

- `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p irohad --lib signer_operation::tests::credential_provider -- --nocapture`: 5/5 passed.
- `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p irohad --lib signer_operation::final_promotion::account_transaction::tests::current_observation -- --nocapture`: 10/10 passed.
- `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p irohad --lib runtime_credential::tests -- --nocapture`: 3/3 passed.
- `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p irohad --lib current_assignment_ -- --nocapture`: 3/3 passed.
- `CARGO_BUILD_JOBS=1 cargo test --offline --locked -p irohad --lib current_evidence_rejects_even_valid_council_and_advert_without_live_finality -- --nocapture`: 1/1 passed.

The first Current Check suite exposed an invalid test assumption: a Check
carrying a deliberately false retained floor cannot apply. The corrected
negative fixture observes the native rejection while simulating a transport
acknowledgement; the runtime still refuses signing state. The isolated test and
complete Current Check suite then passed.

These are component checks. The daemon still has no configured authenticated
`SignerOperationStateSourceV1` joining native Reserve/Complete, phase Checks,
submission, observer signing, qualified UTC, rollback floor, and private
receipt comparison. Topology and other inner promotion approvals likewise lack
their own completed-operation authority. Promotion remains blocked. No HSM is
required for the remaining software-custody work.
