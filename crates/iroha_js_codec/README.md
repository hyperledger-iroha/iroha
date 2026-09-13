# JavaScript canonical codec owner

This library owns the strict account and instruction conversions shared by
JavaScript platform bindings. It uses the actual Iroha account/controller and
Norito implementations and preserves the explicit instruction JSON admission
rules previously owned by `iroha_js_host`.

The public boundary parses and renders account addresses, and encodes/decodes
both public instruction frames and the compact `InstructionBox` archives used
inside transaction payloads. These encodings are distinct. Archive admission
requires exact consumption and canonical native re-encoding.

All four instruction encode/decode operations require an explicit `network_prefix:
u16`. The caller's transaction or network profile owns this value; it is never
inferred from a domainless account archive or defaulted to another network.
The entire synchronous operation runs under the existing thread-local
`ChainDiscriminantGuard`, including typed JSON admission, rendering and canonical
re-encoding. The guard restores the previous context on success, error and unwind;
these operations never mutate the process-wide network selector. Native typed
helper consumers must similarly enter their explicitly selected network scope.

The instruction JSON catalog supports every browser transaction allowlist family:
smart-contract deployment, game, NFT market and Kagemusha top-up. Typed native
owners validate payloads; unsupported envelopes do not fall back to generic JSON.

Native bindings convert buffers and error categories only. They also reuse the
typed helper operations for their existing transaction builders and inspection
APIs. There is no Node, daemon, network, filesystem, or key-custody dependency in
this library's interface. All existing model and account-curve features remain
enabled; this extraction does not qualify a browser artifact or alter platform
support policy.

Focused validation: `cargo test -p iroha_js_codec --lib`. The native host's
`shared_codec_adapter` tests verify platform error and result conversion.
