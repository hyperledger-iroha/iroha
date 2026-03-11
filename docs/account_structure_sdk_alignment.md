# Account/Asset Hard-Cut Alignment

Teams: Rust runtime, Rust/Swift/Android/JS SDKs, bridge/codec tooling.

This repository ships strict first-release semantics. There is no compatibility
path for legacy account/asset literals.

## Required behavior
1. **Account parser contract (strict):**
   - Accept only canonical I105 account literals.
   - Reject all of:
     - any `@domain` suffix
     - alias literals
     - canonical hex account literals in parser input
    - non-canonical/legacy I105 literals
     - legacy `norito:<hex>` account literals
     - `uaid:` / `opaque:` account parser forms
2. **Account identity surface:**
   - Account-facing APIs are domainless and operate on subject identity.
   - Domain context is represented only via explicit scoped/link records where needed.
3. **Asset parser contract (strict):**
   - Accept only encoded asset IDs: `norito:<hex>`.
   - Reject legacy textual forms (`asset#domain#account`, `asset##account`, etc.).
4. **Canonical output:**
   - Render account IDs as canonical I105 in user-facing output.
   - Canonical hex remains debug/render-only and is not accepted as parser input.
5. **Compatibility policy:**
   - No parser fallback branches.
   - No parse-any account/asset entrypoints in runtime SDK paths.

## Validation closure
- Include explicit rejection tests for all forbidden literal forms.
- Keep legacy forms only in negative-path tests.
