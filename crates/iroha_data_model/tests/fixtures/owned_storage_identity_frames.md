# Storage wrapper identities

`owned_storage_identity_frames.json` preserves 432 complete frames across nine
values, 40 nominal identities and six declared layout combinations. It covers
account details, asset quantities, NFT and RWA storage values, and a boxed
string whose root identity already projects to `String`.

Fixture bytes: 1,167,413. SHA-256:
`93a70352e47926baf33b0e64b016d20fa17c1a94885095394acb526ea964b156`.

The capture ran before the `Owned<T>`, `AccountDetails`, `NftData` and `RwaData`
identity declarations were added. Compiler-observed names, both directional
hashes, bare payloads, complete frames and JSON were recorded. The selected
999-input source manifest has SHA-256
`f88b9d6182a5a68be784ce40e65c5b615991963edaefe8de1c0c631e60c853c0`;
those inputs remained unchanged through the capture. This is development
evidence, not a sealed release candidate or complete feature qualification.
The [capture record](owned_storage_identity_capture.json) retains the observed
artifact and harness identities, validation result and scope limitations.
The exact [selected-input manifest](owned_storage_identity_capture_inputs.json)
is retained alongside it so the recorded source hashes remain inspectable
without local build outputs.

`Owned<T>` forwards the inner type's root frame and preserves its bytes.
Generic parents retain the wrapper's distinct nominal identity: vectors,
options and maps must reject the corresponding unwrapped container's header.
The permanent tests compare the declared identities directly with this frozen
capture and check truncation and exact `SchemaMismatch` rejection. A separate
marker-only test requires neither a binary nor a JSON codec, including through
nested wrappers. The temporary capture writer is removed.

The initial layout matrix exposed invalid metadata frames. Entry serializers
were merging default compact flags into explicit fixed-width layouts; the
handwritten packed writer also failed to advertise its offset table. The
capture above follows the correction: inherited field layouts, checked packed
offsets and streamed entry payloads. The prior failing logs remain under
untracked `target/architecture-redesign/owned-storage-identity/`. They are not
accepted frame fixtures. Default tuple/metadata bytes remain covered by the
existing parity tests; no alternate decoding path was added.

The JSON map uses string keys. The separately discovered numeric JSON map-key
writer mismatch remains a distinct correction; this fixture does not qualify
that writer.
