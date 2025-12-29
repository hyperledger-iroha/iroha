# IH-B32 Rollout Note for SDK & Codec Owners

Teams: Rust SDK, TypeScript/JavaScript SDK, Python SDK, Kotlin SDK, Codec tooling

Context: `docs/account_structure.md` is updated with the final IH-B32 address
design, interoperability mappings, and implementation checklist. Please align
library behaviour and tests with the canonical spec.

Key references (line anchors):
- IH-B32 encoding rules & lowercase requirement — `docs/account_structure.md:120`
- IH58 alias procedure & normative vectors — `docs/account_structure.md:151`
- Implementation checklist — `docs/account_structure.md:276`

Action items:
1. Surface IH-B32 as the default textual form in each SDK and remove
   `alias@domain` alias parsing so all inputs go through the canonical codec.
2. Implement CAIP-10 and IH58 conversions exactly as documented.
3. Add test coverage using the normative vectors (Ed25519 + secp256k1) and the
   checksum/chain-mismatch failure case.
4. Mirror the registry lookup contract: expect Nexus manifests to publish the
   `{discriminant, ih58_prefix, chain_alias}` tuple and enforce TTL semantics.
5. Coordinate release notes so downstream integrators know IH-B32 support ships
   consistently across languages.

Please acknowledge once the codecs/tests are updated; open questions can be
tracked in the account addressing RFC thread.
