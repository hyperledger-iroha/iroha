# Native recovered owner

The Rust provisioner consumes a concrete `KagemushaAuthenticatedCoreOwnerV1`,
its independently installed native P256 authorization key and exact storage path.
The owner retains authenticated production recursion, hardware checkpoint
transport and the locked original journals. A decoded snapshot, generic verifier,
application callback, Experimental host or caller checkpoint cannot supply it.
The path-only C/JNI installer selects that retained owner. Open returns an opaque
selection handle; it establishes no account possession or monetary authority.

Method 12 uses the existing `IKGMCOR1` schema-2 frame, little-endian scalar fields,
16 fields maximum, 96 KiB maximum per field, 256 KiB total request and 128 KiB total
response. Initial issuer phases 1–8 retain their separate ceremony semantics.
Recovered phases are:

| Phase | Request fields | Response fields |
| --- | --- | --- |
| 9 | `9:u32` | nonzero `attempt:u64`, canonical account challenge (1–16384 bytes), account signing message (32 bytes), canonical operation-1 command (1–2048 bytes), nonzero device request identity (32 bytes) |
| 10 | `10:u32`, original nonzero `attempt:u64`, original Ed25519 account signature (64 bytes), complete original `IKGMJRS1` success frame (1–`KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1` bytes, also subject to the frame field limit) | original `attempt:u64` |
| 11 | `11:u32`, original nonzero `attempt:u64` | empty |

Phase 9 creates one OS-random device nonce and a 120000 ms suspend-inclusive native
deadline. It retains the exact account, FI, dataspace, network, asset incarnation,
lane, complete freshly selected Core checkpoint and original terminal-certificate
digest. The account signs the returned native `HashOf` signing message. The device
executes the returned exact operation-1 command under the returned request identity.
Phase 10 independently verifies both signatures and their complete original
transcripts, epoch, key, release and credential floors. It freshly reauthenticates
the full hardware checkpoint and held journal prefixes before installing a
revocable observation session under the same original owner.

An exact phase-9 retry reads the original challenge without renewing its deadline.
An exact successful phase-10 retry returns the original acknowledgment while its
session and current owner remain valid. Substituted input, stale checkpoint,
credential regression, deadline loss, uncertain publication or source drift cannot
renew a lease. Phase 11 cancels the original pending attempt; Close revokes its
pending attempt or completed session. No second attempt starts under that Open.
Native registry ownership serializes calls and preserves revocation during queued
hardware work.

After possession, this adapter grants only the actual native observation methods
2, 3 and 11. It does not turn recovery possession into new monetary work, KYC
renewal, ledger inclusion or a fabricated wallet bootstrap. Monetary dispatch must
remain bound to the separately authenticated concrete Core machine and its actual
paired proofs, nonforking device transactions and original native journals.
