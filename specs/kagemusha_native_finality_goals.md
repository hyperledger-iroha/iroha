# KAGEMUSHA native finality replacement

## Decision

KAGEMUSHA Load finality is verified directly from Sumeragi BLS commit
certificates, rooted in the application's independently authenticated signed
genesis and authenticated validator changes. A finality prover, recursive block
history, finality circuits and finality proving/verifying-key inventories are
not part of the first release. Retired implementations and formats are deleted;
there is no compatibility mode or fallback decoder.

The existing production profile trusts the released wallet application on a
stock uncompromised OS and its nonexportable payment key. The native wallet
must authenticate the exact successful Load receipt before requesting its signed
Advance. The monetary relation binds that receipt, its amount and ordinal, the
state transition, replay insertion and the credential-bound Advance signature.
It does not prove BLS verification inside PLONK. A modified application is outside
this selected profile; accepting a host boolean is not a replacement for the
mandatory native verification before signing.

## Goals

| ID | Owner | Outcome and completion criteria |
| --- | --- | --- |
| NF1 | Data model / Sumeragi | One bounded native certificate format and verifier. Pin network and global genesis scope; authenticate validator keys and proofs of possession, exactly `n - f` votes, epoch transitions and their activation heights, certified execution result and counted receipt-event inclusion. Reject malformed signatures, wrong roots, stale/substituted committees, omitted transitions, foreign receipts, trailing data and oversized inputs. |
| NF2 | Native wallet / SDK | Verify evidence before any Load Advance signature or balance mutation. Persist exact receipt/evidence for recovery, repeat verification on replay/import, bind all Load terms and refuse unavailable or invalid evidence without signing. Both mobile SDKs use the same native owner. |
| NF3 | Proofs / artifact owners | Remove the recursive finality source and Load finality-verifier stage while preserving receipt binding, replay protection and authenticated Advance signatures. Rebuild the affected A/W/Omega catalog; no finality proof, accumulator, descriptor, proving key or verifier-key dependency remains. |
| NF4 | Torii / config / tools | Serve ordinary native receipt evidence with no finality worker, proof journal, finality artifact compiler, server provisioning or large-disk requirement. Delete retired configuration and packaging fields and reject their use. Register continues through native finality. |
| NF5 | Integration / release | Pass focused model, proof, preparation, server and bridge tests, then genuine Load, restart, offline A-to-B-to-C and Unload on a four-validator network. Measure certificate bytes, verification latency and phone memory on the current candidate; do not transfer historical recursive-finality qualification or assert unmeasured millisecond/few-kilobyte results. |

## Validation boundary

Implementation and focused tests do not establish physical-phone performance,
the complete installed monetary proof catalog or release qualification. The
remaining evidence belongs to the current candidate. Native state synchronization
must preserve authenticated schedule continuity; an arbitrary validator list or
untrusted checkpoint is never finality authority.
