# Generic model identity fixtures

These immutable frames were captured before adding the ten identity declarations.
The capture used the pinned repository toolchain, locally resolved model-library test
features, and the default stack. Both capture tests passed with all 5,611 selected
inputs unchanged. Temporary capture writers have been removed from the model.

| Fixture | Scope | SHA-256 |
| --- | --- | --- |
| `model_generic_identity_frames.json` | 68 populated frames: 12 generic families and five concrete-argument cases, each as root, Vec, Some and BTreeMap | `a54a824b4c6006dc022cba9306ed722cb4acdfc6dd4c8187442fba75da9e0cf9` |
| `fhe_provenance_identity_frames.json` | Five actual FHE signing preimages | `70abb7bee6935e909fa22e5fc58e5c8794c23a7ad387f5317f60ea48390dc2ed` |

The generic owners are MetadataChanged, Validate and Mismatch. Their recorded names
include the actual private model scopes. Concrete argument declarations cover RwaId,
SignedTransaction, AnyQueryBox and TransactionDomain, including both domain variants.
TriggerId and InstructionBox names are captured inside their generic frames. The
instruction root declaration preserves its existing wire-ID/payload-pair projection;
its containers retain the nominal instruction name. A separate root test checks both
codec directions, roundtrip bytes, truncation and incorrect-header rejection.
Metadata covers all seven current target aliases. Executor inputs include nonempty
instructions, a query and a deterministic signed transaction with a fixed timestamp.
These values demonstrate codec behavior; they do not establish transaction acceptance.

`actual_type_name` records the original compiler name. Permanent tests compare the
explicit nominal declaration against that saved value, allowing physical source moves
without changing the declaration. The tests compare both directional hashes, bare
payloads and complete frames, and exercise decoding/re-encoding. Wrong headers and
truncated inputs are rejected. Marker-only checks require no payload codecs; equal
String/Box<str> payloads retain distinct nominal arguments inside generic frames.

The private borrowed FHE helper is encoding-only. Its captured name retains the erased
lifetime slot. The initial attempted decoder-hash capture failed because the borrowed
fields cannot meet the derived higher-ranked decoder bound; no decoder hash or owned
roundtrip is claimed. After the successful encoding capture, that unusable Decode
derive was removed. The tests compare every frame to the public signing-preimage
function and retain distinct ordered/reversed execution-proof cases. The supplied
proof records are codec fixtures, not verified cryptographic evidence.

Five model identity tests and four adjacent query identity tests pass on one rebuilt
default-feature artifact, with all 5,613 selected inputs unchanged. The same artifact
passes ten participant-settlement tests and the removed recursive-layout rejection
test with no stack override. These declarations do not switch active codec dispatch.
Remaining owner coverage, other feature selections, the atomic trait cutover and
physical model extraction remain required.
