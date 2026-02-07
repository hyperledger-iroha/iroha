---
lang: hy
direction: ltr
source: docs/account_structure_sdk_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 164bd373091ae3280f9f90fcfd915a90088b0c79b8f3759ffd2548edb64d0a90
source_last_modified: "2026-01-28T17:11:30.632934+00:00"
translation_last_reviewed: 2026-02-07
---

# IH58 Rollout Note for SDK & Codec Owners

Teams: Rust SDK, TypeScript/JavaScript SDK, Python SDK, Kotlin SDK, Codec tooling

Context: `docs/account_structure.md` now reflects the shipping IH58 account ID
implementation. Please align SDK behaviour and tests with the canonical spec.

Key references:
- Address codec + header layout — `docs/account_structure.md` §2
- Curve registry — `docs/source/references/address_curve_registry.md`
- Norm v1 domain handling — `docs/source/references/address_norm_v1.md`
- Fixture vectors — `fixtures/account/address_vectors.json`

Action items:
1. **Canonical output:** `AccountId::to_string()`/Display MUST emit IH58 only
   (no `@domain` suffix). Canonical hex is for debugging (`0x...`).
2. **Accepted inputs:** parsers MUST accept IH58 (preferred), `sora` compressed,
   and canonical hex (`0x...` only; bare hex is rejected). Inputs MAY carry an
   `@<domain>` suffix for routing hints; `<label>@<domain>` aliases require a
   resolver. Raw `public_key@domain` (multihash hex) remains supported.
3. **Resolvers:** domainless IH58/sora parsing requires a domain-selector
   resolver unless the selector is implicit default (use the configured default
   domain label). UAID (`uaid:...`) and opaque (`opaque:...`) literals require
   resolvers.
4. **IH58 checksum:** use Blake2b-512 over `IH58PRE || prefix || payload`, take
   the first 2 bytes. Compressed alphabet base is **105**.
5. **Curve gating:** SDKs default to Ed25519-only. Provide explicit opt-in for
   ML‑DSA/GOST/SM (Swift build flags; JS/Android `configureCurveSupport`). Do
   not assume secp256k1 is enabled by default outside Rust.
6. **No CAIP-10:** there is no shipped CAIP‑10 mapping yet; do not expose or
   depend on CAIP‑10 conversions.

Please confirm once the codecs/tests are updated; open questions can be tracked
in the account-addressing RFC thread.
