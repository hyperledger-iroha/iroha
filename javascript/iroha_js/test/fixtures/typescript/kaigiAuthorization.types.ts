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

const usageOptions: import("../../../index.js").KaigiUsageProofOptionsV1 = {
  networkId: options.networkId, callId: options.callId, hostId: options.hostId,
  preRosterRoot: options.preRosterRoot, segmentIndex: 0xffffffff,
  durationMs: 18446744073709551615n, billedGas: 18446744073709551615n,
  hostCommitment: proof.commitment, blinding: new Uint8Array(32),
};
const {buildKaigiUsageProofV1} = await import("../../../index.js");
const usage: import("../../../index.js").KaigiUsageProofV1 = buildKaigiUsageProofV1(usageOptions);
const usageFromCrypto = (await import("../../../crypto.js")).buildKaigiUsageProofV1(usageOptions);
const usageOutputs: Uint8Array[] = [usage.hostCommitment, usage.usageCommitment, usage.preRosterRoot, usage.proof, usageFromCrypto.proof];
void usageOutputs;
buildKaigiUsageProofV1({...usageOptions,
  // @ts-expect-error Exact u64 metrics require bigint.
  durationMs: 1,
});
buildKaigiUsageProofV1({...usageOptions,
  // @ts-expect-error Exact u64 metrics require bigint.
  billedGas: 1,
});
buildKaigiUsageProofV1({...usageOptions,
  // @ts-expect-error u32 segment uses an exact integer number.
  segmentIndex: 1n,
});
buildKaigiUsageProofV1({...usageOptions,
  // @ts-expect-error Host commitment uses raw field bytes.
  hostCommitment: "11".repeat(32),
});
// @ts-expect-error Stored host C is mandatory.
buildKaigiUsageProofV1({networkId: options.networkId, callId: options.callId, hostId: options.hostId,
  preRosterRoot: options.preRosterRoot, segmentIndex: 1, durationMs: 1n, billedGas: 0n, blinding: options.blinding});
