# Concrete model identity fixtures

These files pin the current native concrete identities and frames. The capture
configuration enables governance and HTTP and disables `ids_projection`;
those fields describe the fixture provenance.

| Fixture | Scope | SHA-256 |
| --- | --- | --- |
| `model_concrete_identity_frames.json` | 11 populated families, each as root, Vec, Some and BTreeMap: 44 frames | `b6d3930c4993338de7f58f259377877f739f557b82a9bc2527fb7274e07c9292` |
| `block_message_send_identity_frame.json` | One encoding-only block-message adapter and its owned decoding projection | `dabcc4eeb99658dac1000f835a761d14a53ef50b0591284543a46fd1a8f7cd01` |
| `reputation_event_id_identity_frames.json` | Two encoding-only reputation event-ID projections and their owned decoding material | `dda98fd79ef7371d004bb16c3d9d513004e45dc5ce1b243fc4ae853b6437432f` |

The five concrete owners are Action, DataEvent, ExecutorContext,
BlockSubscriptionRequest and BlockMessage.
Actions cover schedules with and without retry and explicit execution. Data events
cover peer addition, account metadata, GameSession and Governance. The executor
context contains populated authority/header values. Stream cases cover two
requested heights and a deterministic signed block with transaction results. The block uses the
current external transaction intent and complete typed network output, including
the explicit absent Nexus fee receipt. Retired transaction admission intent and
lane finality result fields are absent from its sole canonical layout.

`actual_type_name` retains the compiler name observed before declaration, including
private model scopes. The permanent tests compare each declared nominal name to
that saved name, both codec directions to the declared frame hash, and every bare
payload, complete frame, frame digest and header flag to the original record.
Roundtrips, incorrect schema headers and truncated frames are also checked.
Explicitly ignored native fixture producers retain capture provenance; ordinary tests compare against the checked-in frames.

The context and signed genesis fixture builders share explicit pins for all five
confidential-feature fields. The policy hash is fixed codec input, independent of
the current genesis-policy default. Direct regressions compare the captured
context header and the final stream-block header and signature; both the genesis
proposal and result-bearing block check all five fields. The separate
`block_header_defaults_confidential_digest` and `genesis_defaults_confidential_digest`
tests cover the current production default. Updating that default must not
silently change the input to these identity fixtures.

BlockMessageSend delegates its root frame to BlockMessage. The private borrowed
reputation adapter delegates to ReputationJournalEntryV1 after clearing the event
ID. These adapters remain encoding-only: the tests decode through their owning
types, compare exact bytes and retain the domain-separated event-ID assertion.
No adapter decoder or alternate frame format is introduced.

Feature selections reuse the same canonical common frames. Only the Governance
family is omitted when governance is disabled; the three HTTP families and the
block adapter require HTTP. The disabled-governance test also rejects the captured
Governance frame. A separate pre-declaration consumer capture exposed GameSession
shifting from tag 22 to 21 when governance was absent. That shifted frame is not a
canonical fixture. DataEvent now reserves all 23 default discriminants; the schema
test checks every present name/tag pair and preserves the absent Governance slot.

These tests qualify identity and codec behavior, not transaction acceptance,
bridge proof verification, guest ABI delivery or release readiness.

The default/HTTP model-library run passes 3,593 tests with zero failures and six
existing fixture generators ignored; all 5,617 selected inputs remain unchanged.
The same three public tests pass in a normal-dependency consumer with governance
actually disabled, using locked/offline Cargo and unchanged package versions,
features and edges. All 5,624 selected inputs remain unchanged for that run.
Both runs unset `RUST_MIN_STACK`. The isolated consumer checks all 48 common
root/container frames, the block-send projection, all 22 available schema
variants and rejection of the reserved Governance frame. These are local codec
checks; complete release qualification remains open.

On 2026-09-12, the native fixture producer refreshed contexts containing the
current default confidential-policy hash. The root payload changes were checked
to consist solely of that hash; outer frame checksums follow from the changed
payload. These synthetic frames do not qualify a network run.

The HTTP block fixture was also refreshed through the native signed-block
constructor. Its bare bytes differ only at the 32-byte policy hash and resulting
64-byte block signature. All other bytes, entrypoint commitments and result
commitments are unchanged, and the new signature verifies.

On 2026-09-26, `SignedBlock` gained its trailing `commit_certificate` field
(Sumeragi finality proof; absent from the fixture block). The native producer
refreshed only the block-message family and the HTTP block-message adapter: each
frame gains the encoded `None` field and its length prefixes and checksums follow.
All other families are unchanged.

On 2026-09-28, the unused `TriggerContext` payload was removed together with the
retired Rust trigger SDK, and its `trigger-context` family (four frames) was
dropped from the capture. The same native-producer refresh follows the committed
`BlockHeader` extension with the trailing required `global_beacon_pulse_hash`
option (absent in these fixtures): `smart-contract-context` and
`executor-context` gain two bare bytes, and `block-message` and the block-message
adapter gain four. Nominal names and schema hashes are unchanged, and every other
family is byte-for-byte unchanged. Frame counts in the earlier run summaries
above predate this refresh.

Also on 2026-09-28, the unused `SmartContractContext` payload was removed together
with the retired Rust smart-contract SDK, and its `smart-contract-context` family
(four frames) was dropped from the capture. The captured-header regression now
decodes the `executor-context` frame. Every remaining family is byte-for-byte
unchanged.
