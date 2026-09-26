# F02 borrowed canonical ingress binding, 2026-09-24

Scope: the existing `optimizations` checkout. The canonical executed-body
worker already receives an `InboundBlockMessage` holding the original
authenticated message, reply route, and fair-ingress evidence. Its binding
check previously deep-cloned the recovery request into a second `BlockMessage`
only to ask whether that evidence matched the message. It now compares the
evidence with a borrow of the original inbound message before moving any
owner. A changed-message test attaches fair-ingress evidence, alters the
message, and requires exact-binding refusal with the original carrier returned.

This removes one duplicate request clone. The separate canonical request
frame-size check still clones and encodes a request before physical admission,
and `matches_message` still encodes a comparison buffer. Their allocation,
original-owner capacity, live ingress, and canonical response wiring remain
open. The worker is not production-connected by this cut.

The current-source Core `canonical_executed_body_worker_` selector passes
3/3, including the changed-message owner-return control and existing source,
route, kind, frame, and worker-capacity cases. Scoped Rust formatting and
diff checks pass. F02 and F03 release gates remain open.
