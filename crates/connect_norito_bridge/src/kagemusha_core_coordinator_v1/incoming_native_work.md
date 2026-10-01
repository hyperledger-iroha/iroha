# First-release incoming native work

ABI25, coordinator frame `IKGMCOR1` schema2. Codes15–17 are separate from method12 enrollment phases. Every call requires the independently installed concrete Core owner and its live, account-and-device-authenticated recovered possession lease. A public credit ID, a generated pair, an app journal, or a lifecycle aggregate reply grants no owner.

The shared frame has at most16 fields, each length-prefixed by a little-endian u32. Request256KiB and response128KiB ceilings remain. Digests are32 bytes and nonzero; integer fields have the exact sizes below. No path, clock, state nonce, authority policy, root, signer, provider or Core owner enters through C/JNI.

Method17 `StageIncomingOriginal` request is `[kind:u32LE, creditID32]`, kind0=reserveMint,1=stageMint,2=stagePeer. Response is `[sameCreditID32]` only after genuine native staging and hardware checkpoint publication. The immutable Rust work source supplies exact original typed reservation/certificate, governed mint credit/stage certificate, or peer request/payment/recipient-private opening/native staging instant/certificate/acknowledgement. The concrete Core independently verifies every original. Reservation publication precedes online debit; mint/peer stage publication precedes fold; peer acknowledgement must not leave custody before native stage publication. A duplicate must resolve the same original, never create another physical stage.

Method15 `PrepareIncomingFold` request is `[kind:u32LE, creditID32]`, kind0=MintFold,1=ReceiveFold. Native custody chooses original nonce, trusted time and paths, consumes the usable Core owner and fsyncs the original complete intent before hardware work. The genuine production prover uses the opaque exclusive fold selection, independently authenticated signed artifacts and physical private-witness custody. Its exact paired proof is authenticated and fsynced in the same original journal before response or physical work exposure. An existing original pair is reused; it is never regenerated after exposure.

Method15 response fields in order:

1. actual native history operation ID32;
2. exact credit ID32;
3. canonical PUBLIC `HardwareTransitionStatementV1` (1..8192 bytes);
4. exact native proof-statement digest32;
5. normalized Guard digest32;
6. original domain-separated history root-selection signing bytes (1..32768 bytes);
7. actual selected device key reference32;
8. hardware epoch generation:u128LE16, nonzero;
9. exact hardware epoch ID32;
10. exact canonical original generated `KagemushaPairedProofV1` (1..8192 bytes; model pair6528-byte bound remains independently enforced).

The private successor, plaintext credit opening, sparse-tree witness, recovery seed and complete `TransitionPreviewV1` never cross this boundary. The 128KiB complete response ceiling also applies.

Method16 `CompleteIncomingFold` request is `[sameHistoryID32, sameOriginalCanonicalPair(1..8192), originalCanonicalHardwareTransitionCertificate(1..96KiB), originalLowSP256DeviceRootSignature64]`. The signature covers the exact method15 root-selection transcript under the independently selected DEVICE key. A lifecycle response authenticator, Core authorization signature or aggregate projection cannot substitute it. The required qualified physical incoming evidence owner must retain/recover this exact original operation and return the original Guard/certificate and root signature; C/JNI cannot register this owner. Native custody compares the returned originals and independently verifies actual State proof, Guard, device key, native history CAS and original checkpoint before publishing funds. There is no new OEM opcode inferred from existing op17/18 aggregate commands.

Method16 response is `[sameHistoryID32]` only after native history commit and fsynced successor hardware checkpoint publication complete. A fresh authenticated current-state read is a separate operation. Every completion retry keeps the same original pair, Guard, signature, history ID and native destination/checkpoint ID. Uncertain work retains the exclusive cap/publication; no usable predecessor escapes and no second hardware fold starts. Prepared work is unavailable after completion begins. Closing/cancelling the managed lease prevents dispatch but cannot roll back irreversible hardware; restart must recover original native journals and independently reauthenticate ownership. Generic failure never falls back to initial enrollment or an aggregate-only balance path.

Required physical inputs are the actual qualified nonforking hardware journal, selected device private key/root-selection service, trusted native ledger/time, original sealed witness/unseal service and authenticated signed PK/VK/release inventory. Software cannot manufacture these inputs. Their absence returns unavailable.
