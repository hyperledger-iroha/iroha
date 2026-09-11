# Versioned JSON ownership

The isolated candidate removes the unused JSON-helper export facade. Versioned
JSON methods require an explicit immutable Norito context. The generated decoder
borrows the discriminator and content from its bounded native envelope; it no
longer clones the content tree. The serializer writes a borrowed envelope directly
through the checked writer, preserving the previous content-before-version order.

JSON errors retain their typed source, including missing context, resource,
allocation and depth failures. Unsupported-version errors hold their diagnostic
inline. Their raw JSON or Norito bytes are admitted before one fallible copy;
the extra diagnostic box is removed. Binary encoders, schema identities and
captured wire fixtures remain unchanged. The two explicit block/Torii diagnostic
constructors use the same capture owner; their containing suites remain pending.

Source `c776686f4896e9d4104ca72684bbaac6192bdbff5893681aa09af7ac5207f02e`
passes both packages' complete selected test build and 22 tests across five
executables. This includes five wire-fixture tests and all eight compiler UI
cases. Strict all-target Clippy and the executable documentation example pass.
The source remains unchanged across these runs, with ordinary worker stacks.

New regressions verify context forwarding, exact field ordering, borrowed
addresses, envelope error precedence, exact/short diagnostic-copy budgets and
typed resource/depth errors. A 32,768-level wrong envelope is rejected and
ordinarily dropped without recursion overflow.

The first UI run exposed two fixtures missing the required JSON derives and
five stale diagnostic snapshots. The fixtures now declare the complete contract;
each negative diagnostic was reviewed against its unchanged input and current
compiler output. Initial failures and the subsequent naming/test lint corrections
remain recorded. No blanket diagnostic overwrite or lint allowance was used.

Reports and reviewed source stages are under
`target/architecture-redesign/model-base-extraction-v1/`; the final scoped result
is `norito-context-asset-composed-v1/version-context-closeout-v1.json`.
Reproducible compiler UI caches were removed after the final suite passed;
source, snapshots and result identities remain.

TODO: Complete SoraFS manifest and aggregate JSON consumer migration, then
qualify the combined candidate. These scoped results do not establish full
workspace, native/device, network or release qualification.
