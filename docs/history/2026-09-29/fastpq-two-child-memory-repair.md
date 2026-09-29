# FASTPQ two-child memory repair: September 29 evidence

The normal optimized public ordinary and AXT producers both complete after
physical-source release and retained Metal-pool admission. Each complete output
passes a separate process that verifies the retained public artifact without
constructing a witness or proof. Default construction payload (2 GiB), structural
work (2^42), child (502,895 admitted bytes, below 512 KiB), outer artifact (1 MiB)
and query (128 total) bounds remain unchanged.

| Route | Artifact bytes | Construction and self-verification | Whole test wall time | Peak process RSS | Separate replay wall / RSS |
| --- | --- | --- | --- | --- | --- |
| Ordinary | 971,675 | 4,438.147265958 s | 4,447.99 s | 1,881,849,856 B | 10.28 s / 28,639,232 B |
| AXT | 973,573 | 4,014.031983708 s | 4,023.62 s | 1,875,820,544 B | 8.84 s / 32,227,328 B |

The ordinary SHA-256 is
`7c6ce4cced9332faa4d2af0f1fffc1c3c16b563197fca1cff8bbd804c0350b50`;
the AXT SHA-256 is
`968225a24c89e08051afa5ac19881c6d7371a3329f08a1e044497fa31e722f74`.
Both run from immutable API executable
`a3952ef7ed707a5af9af0ca2f3001915402c62c970305d4be8cd3a0de7c4a68f`,
built normally at actual opt-level 3 with `default+fastpq-gpu`. Required Metal
executes digest work; arithmetic still uses the CPU. This contended host run
does not establish a latency promise, speedup, device-fleet qualification, or
independent cryptographic review.

Receipts, public artifacts, generation/replay logs, exact test inventories and
immutable executable are retained under
`dist/zk-remediation/2026-09-29/fastpq-source-owner-proof-run1/two-child/`.
The terminal `complete-receipt.json` records both passes and only `Cargo.lock`
post-capture drift relative to the 225-file scoped source manifest
`12c0640f2138927693377e84d69550434b0fb813ba032dc62efb76f1deb89090`.
That scope is FASTPQ/ISI/build inputs, not the full dependency closure, and does
not qualify subsequent AXT source-occurrence API edits in the shared checkout.

The prior ordinary run's 2,174,222,336-byte RSS overrun remains preserved in the
[September 28 record](../2026-09-28/fastpq-masked-native-validation.md). The
current process measurements fix that observed outcome for these exact fixtures.
Charged construction payload and measured process RSS are different quantities;
passing either alone does not prove a universal RSS bound.

Both fixtures contain two segments and four chronological updates, with near
maximum signed 512-bit sender values and a scale-28 receiver. They repeat two
account keys. Current follow-up tests add four distinct canonical account keys
spanning all four high-bit path quadrants (127 retained nodes, 128 siblings,
251 touched-node hashes) and the largest canonical AXT context admitted by the
unchanged outer preflight. Those additions have not yet completed native checks
or full proofs. Their fixture builds source-occurrence correspondence before
sorting remote claims, preserving the current AXT occurrence API.
