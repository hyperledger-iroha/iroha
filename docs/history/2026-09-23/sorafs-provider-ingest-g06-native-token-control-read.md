# SoraFS G06 native source-token control read

2026-09-23, `optimizations`. The daemon's provider-ingest query now has a
read-only [source StreamToken control observation](../../../crates/irohad/src/sorafs_provider_ingest_finalized_query/current_source_stream_token_custody.rs).
It starts with the qualified current source assignment, reads the existing
[native StreamToken custody control](../../../crates/iroha_core/src/query/stream_token_custody.rs)
from one committed State view bound to the same Kura, and requires the State
height/hash to equal the archive head. It repeats the assignment read and
archive-generation check after the native read. The result pairs exact network,
source, assignment revision and head with the native role-state digest and
public control. It rejects a missing control, unenrolled head, revoked signer or
attester, substituted source/binding, changed key policy and stale head.

This is one genuine finalized input for a future independently administered
resolver. The public binding uses the software-capable StreamToken role and is
checked against native policy; it does not assume hardware custody. The result
does not verify the signed custody statement against independent attestation
trust or current time, and cannot issue or validate a source grant. The
[current-authority seam](sorafs-provider-ingest-g06-grant-authority-seam.md)
still lacks a finalized council admission/renewal/revocation and signed-advert
producer, historical admission-cursor ancestry, and governed HTTPS origin,
token-verification-key and DER-root pins. The HTTPS leaf still requires an
injected grant resolver and remains fail-closed.

The focused [join tests](../../../crates/irohad/src/sorafs_provider_ingest_finalized_query/current_source_stream_token_custody_tests.rs)
cover the exact head, source and public control plus stale head, changed native
anchor, substituted source/network, revoked signer and attester, absent active
head, and stale key generation. The existing missing-State/Kura-head test now
calls the producer and checks unavailable finality and early wrong-source
rejection. The fresh daemon library build passed
`source_stream_token_control` **2/2**, and the same binary passed
`current_assignment_lookup_fails_closed_without_head_and_preserves_worker_cursor`
**1/1**. The
candidate still needs a live committed State/Kura transition test for native
revocation and key rotation, then a four-validator ingest/restart run after the
missing governance producer and grant resolver exist. G06 and promotion remain
open. No compatibility path was
added.
