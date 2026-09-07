import {
  buildKaigiAuthorizationProofV1,
  NetworkId,
  type KaigiAuthorizationProofOptionsV1,
  type KaigiAuthorizationProofV1,
} from "../../../index.js";
import { buildKaigiAuthorizationProofV1 as cryptoBuilder } from "../../../crypto.js";

const options: KaigiAuthorizationProofOptionsV1 = {
  networkId: NetworkId.fromBytes(new Uint8Array(32)),
  callId: { domainId: "wonderland.sora", callName: "weekly-sync" },
  hostId: "canonical-original-host",
  subjectId: "canonical-original-participant",
  participationSequence: 18446744073709551615n,
  action: "leave",
  preRosterRoot: new Uint8Array(32),
  blinding: new Uint8Array(32),
};
const proof: KaigiAuthorizationProofV1 = buildKaigiAuthorizationProofV1(options);
const subpathProof: KaigiAuthorizationProofV1 = cryptoBuilder(options);
const outputs: Uint8Array[] = [proof.commitment, proof.nullifier, proof.authorization,
  proof.preRosterRoot, proof.proof, subpathProof.proof];
void outputs;

buildKaigiAuthorizationProofV1({ ...options,
  // @ts-expect-error JavaScript numbers do not preserve an exact u64.
  participationSequence: 1,
});
buildKaigiAuthorizationProofV1({ ...options,
  // @ts-expect-error NetworkId is a typed, validated network binding.
  networkId: new Uint8Array(32),
});
buildKaigiAuthorizationProofV1({ ...options,
  // @ts-expect-error The four actions form a closed vocabulary.
  action: "Join",
});
buildKaigiAuthorizationProofV1({ ...options,
  // @ts-expect-error Secret seeds are not authorization witnesses.
  seed: "old seed",
});
buildKaigiAuthorizationProofV1({ ...options,
  // @ts-expect-error The consumed blinding must be mutable bytes.
  blinding: "01".repeat(32),
});
// @ts-expect-error The retired roster builder is absent.
type RemovedBuilder = typeof import("../../../index.js").buildKaigiRosterJoinProof;
// @ts-expect-error The retired roster proof result is absent.
type RemovedProof = import("../../../index.js").KaigiRosterJoinProof;

const commitment = { commitment: proof.commitment };
const nullifier = { digest: proof.nullifier };
const leave: import("../../../index.js").LeaveKaigiInput = {
  callId: { domainId: "wonderland.sora", callName: "weekly-sync" },
  participant: "canonical-current-signer", commitment, nullifier,
  rosterRoot: proof.preRosterRoot, proof: proof.proof,
};
void leave;
const retiredHint: import("../../../index.js").KaigiParticipantCommitmentInput = {
  commitment: proof.commitment,
  // @ts-expect-error Even null retired hint fields are absent from final V1.
  aliasTag: null,
};
const retiredTimestamp: import("../../../index.js").KaigiParticipantNullifierInput = {
  digest: proof.nullifier,
  // @ts-expect-error Issuance-time hints are absent from final V1.
  issuedAtMs: 0,
};
void [retiredHint, retiredTimestamp];
