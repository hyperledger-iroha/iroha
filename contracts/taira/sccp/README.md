# SCCP Taira settlement

SCCP settlement is native Iroha Core functionality. No Taira-side contract is
deployed for outbound locking or inbound release.

Each immutable governed route revision contains a typed canonical SORA
settlement asset, custody account, and the exact decimal scale used by SCCP
payload amounts. The settlement tuple is part of the route configuration
commitment and cannot be changed beneath durable message history.
`RecordSccpMessage` atomically transfers an outbound amount from the transaction
authority to that custody account before it creates the outbox record. A fully
verified native inbound proof atomically transfers the same governed asset from
custody to the canonical SORA recipient before the proof and durable replay
record are stored.

Failed transfers create no outbox, proof, or replay record. The native paths use
the ordinary asset-transfer policy and accounting machinery, so settlement does
not depend on IVM execution mode, sibling burn instructions, or an empty
contract call.
