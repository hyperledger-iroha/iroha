# Historical SoraFS status excerpt

Preserved verbatim from `status.md` in the isolated final-promotion candidate
before the native deployment-authority checkpoint. These local observations
remain historical and do not qualify the later candidate.

Excerpt SHA-256: `caaaf4b78f978d1f7fc3f5db43b2abf7eaab4c944d529a6404340af079083e92`.

SoraFS follows the [V1 implementation goals](specs/sorafs/v1_implementation_goals.md)
and the [September 12 closure checkpoint](specs/sorafs/v1_closure_ledger.md#2026-09-12-native-custody-reconciliation-and-final-promotion-work).
The isolated final-promotion candidate passes all **1,048 Manifest library tests**,
**10 CLI receipt tests**, and **111 daemon signer tests**, with zero failures or
ignored cases in those selections. All **30 cosign cryptographic regression cases**
pass on unchanged captured sources and fixtures; mandatory Linux CI requires that
exact case set with an independently authenticated, pinned cosign executable.
Workspace formatting and the retired-codec guard pass; compiler warnings remain.
The final checker requires canonical hardware custody evidence and rejects software
promotion. Cosign verifies the exact canonical subject, certificate, Rekor 2 proof
and signed timestamp; contradictory duplicate claims are rejected. Promotion stays
blocked pending all four inner hardware approval proofs. Production signing and
hardware/state adapters, full workspace/SDK checks,
four-validator qualification, all 17 genuine lanes and load/soak remain open.
Two unfinished-source inventory guards still fail. Earlier reboot-era observations
are retained as a [historical excerpt](docs/history/2026-09-12/sorafs-status-before-final-promotion.md).

