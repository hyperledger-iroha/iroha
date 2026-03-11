---
lang: az
direction: ltr
source: docs/source/references/address_norm_v1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8cb00defbc321d11300d4670a4a084e3428c2cccf2206aa6ec2a1c1dc0002abd
source_last_modified: "2026-01-28T17:11:30.739412+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Domain Normalisation v1 (Norm v1)

This note captures the canonical domain-name normalisation pipeline for the
account address envelope. It corresponds to roadmap item **ADDR‑1b** and
documents exactly how Torii, the data model, SDKs, and tooling normalise domain
labels before they are encoded into an `AccountAddress`.

Norm v1 applies to every place where [`Name`](../../../crates/iroha_data_model/src/name.rs)
is parsed under a domain context:

- domain registration (`RegisterDomain` / `NewDomain`);
- implicit-domain address handling (default domain binding);
- manifest and metadata tooling that publish domain references;
- CLI, SDK, and governance interfaces that accept domain identifiers.

The goal is to guarantee that a given Unicode input string always maps to a
single canonical byte representation. Any string that fails one of the steps
below must be rejected deterministically at the boundary where it is provided.

## Configuration summary

| Stage | Implementation anchor | Parameters / knobs | Failure surface |
|-------|-----------------------|--------------------|-----------------|
| Input validation | `Name::validate_str` (`crates/iroha_data_model/src/name.rs`) | Reject whitespace and the reserved delimiters `@`, `#`, `$`; strings must be non-empty. | `ParseError { reason: ... }` propagates to Torii/CLI/SDKs. |
| Unicode NFC composition | `Name::normalize` (`icu_normalizer::ComposingNormalizer::new_nfc`) | Deterministically compose canonically equivalent sequences before ASCII folding. | Cannot fail; normalizer data ships with the binary. |
| UTS‑46 ASCII folding | `canonicalize_domain_label` (same module) calling `Uts46::to_ascii` | `AsciiDenyList::EMPTY`, `Hyphens::Check`, `DnsLength::Verify`, and the default non-transitional behaviour of `Uts46::new()`. Output is lowered before validation. | `ParseError(ERR_DOMAIN_NORMALISATION)` mapped to `AccountAddressError::InvalidDomainLabel`. |
| Character & label policy | Post-processing inside `canonicalize_domain_label` | Enforce ASCII `[a-z0-9-_]`, forbid empty labels, ensure at least one label when splitting on `'.'`, and reject Latin Extended Additional letters (`U+1E00`–`U+1EFF`). | `AccountAddressError::InvalidDomainLabel`. |
| Length limits | Same helper | Each label 1–63 bytes, FQDN ≤ 255 bytes after UTS‑46 output. | `AccountAddressError::InvalidDomainLabel`. |

Downstream consumers (`iroha_cli`, Torii, SDKs) MUST reuse these helpers instead
of implementing ad-hoc pipelines; this guarantees fixtures, telemetry, and
manifests all observe the same canonical bytes.

## Step-by-step pipeline

1. **Input validation.** Reject empty strings, whitespace, and the reserved
   delimiters `@`, `#`, and `$`. This matches the invariants enforced by
   `Name::validate_str`, so failures surface as `ParseError` instances that Torii
   maps to `AccountAddressError::InvalidDomainLabel("empty")`.
2. **Unicode NFC composition.** Apply ICU-backed NFC normalisation so canonically
   equivalent sequences (for example `e\u{0301}` and `é`) collapse to the same
   representation. This is implemented by `Name::normalize` and must remain the
   first normalisation stage.
3. **UTS-46 processing.** Run the NFC output through UTS‑46 in **non-transitional
   mode** with STD3 guardrails enabled. `canonicalize_domain_label` wires
   `AsciiDenyList::EMPTY`, `Hyphens::Check`, and `DnsLength::Verify` into
   `Uts46::to_ascii`, then lowercases the resulting ASCII to obtain the
   canonical A-label sequence. Inputs that violate STD3 or DNS length rules are
   rejected deterministically with `ERR_DOMAIN_NORMALISATION`.
4. **Length & homograph limits.** Enforce per-label and overall length limits:
   each label (substring between dots) MUST be between 1 and 63 bytes in its
   A-label form, and the full domain MUST be ≤ 255 bytes after UTS‑46 output.
   Additionally, Norm v1 rejects Latin Extended Additional glyphs (`U+1E00`
   through `U+1EFF`) to avoid obscure ASCII look-alikes (“ḷ”, “ṅ”, etc.) in
   mixed-script labels. These limits align with DNS expectations and keep the
   Local digest consistent.
5. **Script policy (future).** UTS‑39 confusable checks and explicit script
   policies remain optional in v1. When enabled, they are applied after UTS‑46
   and before the digest step; failure is fatal. Their rollout is tracked under
   ADDR‑1 follow-ups and will ship with Norm v2 once available.

If every stage succeeds, the normalised domain string is cached as the canonical
value for on-chain storage and address encoding. Any deviation at step 3 or 4
must surface a structured `ParseError` from the parsing boundary (Torii, CLI,
SDK, etc.).

## Canonical examples

| Input string | After NFC | After UTS‑46 (lower-case A-label) | Outcome | Notes |
|--------------|-----------|-----------------------------------|---------|-------|
| `Treasury` | `Treasury` | `treasury` | ✅ accepted | Typical ASCII label collapses to lower-case. |
| `例え.テスト` | `例え.テスト` | `xn--r8jz45g.xn--zckzah` | ✅ accepted | Demonstrates multi-label punycode conversion. |
| `bücher.example` | `bücher.example` | `xn--bcher-kva.example` | ✅ accepted | NFC composes `ü` before punycoding. |
| `xn--caf-dma` | `xn--caf-dma` | `xn--caf-dma` | ✅ accepted | Already canonical; pipeline leaves it unchanged. |
| `a..b` | `a..b` | — | ❌ rejected | Empty label trips the DNS length guard and yields `ERR_INVALID_DOMAIN_LABEL`. |
| `例え..テスト` | `例え..テスト` | — | ❌ rejected | Same empty-label failure as above. |
| `foo-.example` | `foo-.example` | — | ❌ rejected | Hyphen at the end violates STD3/`Hyphens::Check`. |
| `Treasury$default` | `Treasury$default` | — | ❌ rejected | `$` is blocked by the input validation step. |
| `\u{0065}\u{0301}.example` | `é.example` | `xn--9ca.example` | ✅ accepted | Shows NFC collapsing combining sequences before punycode. |
| `例え．テスト` (full-width stop) | `例え．テスト` | — | ❌ rejected | Full-width delimiter fails the ASCII folding stage. |
| `wÍḷd-card` | `wÍḷd-card` | — | ❌ rejected | Contains `ḷ` (Latin Extended Additional), which Norm v1 bans for domain labels. |

These examples should be mirrored in fixtures for Torii and SDKs so that
cross-language implementations converge on the same behaviour. The canonical
bundle in `fixtures/account/address_vectors.json` contains the corresponding
I105 encodings for regression tests.

## Reference pseudocode

The following pseudo-Rust snippet illustrates the normalisation loop. Every
production caller must either reuse `canonicalize_domain_label` or perform
logically equivalent steps.

```text
fn normalize_domain(input: &str) -> Result<String, AccountAddressError> {
    Name::validate_str(input)?;                // whitespace / reserved chars
    let nfc = Name::normalize(input);          // ICU NFC composition
    let ascii = Uts46::new()
        .to_ascii(
            nfc.as_ref().as_bytes(),
            AsciiDenyList::EMPTY,
            Hyphens::Check,
            DnsLength::Verify,
        )
        .map_err(|_| AccountAddressError::InvalidDomainLabel(ERR_DOMAIN_NORMALISATION))?;
    let canonical = ascii.to_lowercase();
    ensure_per_label_lengths(&canonical)?;
    ensure_char_policy(&canonical)?;           // `[a-z0-9-_]`
    Ok(canonical)
}
```

Any deviation (for example, skipping NFC prior to UTS‑46, using transitional
processing, or accepting characters outside the ASCII policy) results in
addresses that do not decode on other peers.

## Error mapping

| Stage | Error code | When it fires |
|-------|------------|---------------|
| Input validation | `ERR_INVALID_DOMAIN_LABEL` | Empty strings, whitespace, or reserved delimiters encountered before NFC. |
| UTS‑46 processing | `ERR_INVALID_DOMAIN_LABEL` | `Uts46::to_ascii` rejects the label (illegal hyphen placement, DNS-length violation, invalid ASCII). |
| Character policy | `ERR_INVALID_DOMAIN_LABEL` | Post-UTS scan finds a character outside `[a-z0-9-_]` or an empty label. |
| Length enforcement | `ERR_INVALID_DOMAIN_LABEL` | Label length <1 or >63 bytes, or total payload >255 bytes. |

## Local digest procedure

Local domains are embedded into the account address header using a truncated
Blake2s MAC. Norm v1 fixes the derivation at:

- key: ASCII string `SORA-LOCAL-K:v1`;
- digest: Blake2s‑256 MAC (`blake2s::Mac`) over the canonical domain string;
- truncation: first 12 bytes of the MAC output (`blake2s-96`).

The helper in `AccountAddress` implements this as:

```rust
const LOCAL_DOMAIN_KEY: &[u8] = b"SORA-LOCAL-K:v1";

fn compute_local_digest(label: &str) -> [u8; 12] {
    let mut mac = Blake2sMac::<U32>::new_from_slice(LOCAL_DOMAIN_KEY)
        .expect("static key with valid length");
    Mac::update(&mut mac, label.as_bytes());
    let mac_bytes = mac.finalize().into_bytes();
    let mut digest = [0u8; 12];
    digest.copy_from_slice(&mac_bytes[..12]);
    digest
}
```

For reference, the canonical digest for `treasury` (after Norm v1) is
`0xB1 0x8F 0xE9 0xC1 0xAB 0xBA 0xC4 0x5B 0x3E 0x38 0xFC 0x5D`, matching the
golden vector exercised in
`crates/iroha_data_model/examples/address_vectors.rs`.

## Validation artefacts

- **Fixtures:** `fixtures/account/address_vectors.json` captures I105,
  i105-default-Sora, and canonical-domain samples for every selector class and is
  regenerated via `cargo xtask address-vectors`.
- **Tests:** `crates/iroha_data_model/tests/account_address_vectors.rs`,
  `javascript/iroha_js/test/address.test.js`, Android/Swift SDK suites, and
  `crates/iroha_torii/tests/address_parsing.rs` (added as part of ADDR‑5) keep
  the pipeline honest in CI.
- **Docs:** This note and §3 of `docs/account_structure.md` are the canonical
  references cited by the roadmap entry **ADDR‑1b**.

## Implementation notes

- `Name` currently performs steps 1–2. UTS‑46 and length enforcement are being
  threaded into Torii and SDK boundaries; once the rollout completes,
  `Name::from_str` will enforce the same rules so every entry point shares a
  single policy.
- `AccountAddress::from_account_id` relies on the digest step above; altering
  Norm v1 requires regenerating the golden compressed vectors in
  `crates/iroha_data_model/src/account/address.rs` tests and updating the
  example fixtures.
- Configuration overrides (`set_default_domain_name`) must run the same
  pipeline, which is why the setter delegates to `Name::from_str`.

## Multisig controller schema (ADDR-1c)

The multisignature controller spec now lives in
[`docs/source/references/multisig_policy_schema.md`](multisig_policy_schema.md),
which documents the CTAP2 map, validation rules, deterministic digest, and
golden fixtures backing ADDR‑1c. Refer to that file when implementing or
auditing multisig payload handling.
