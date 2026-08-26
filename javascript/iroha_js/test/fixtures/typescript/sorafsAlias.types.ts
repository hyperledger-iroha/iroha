import type {
  SorafsAliasCacheDecisionV1,
  SorafsAliasCacheReasonV1,
  SorafsAliasListResponse,
  SorafsAliasManifestStatusV1,
  SorafsAliasRecord,
} from "../../../index.js";

const pending: SorafsAliasManifestStatusV1 = { state: "pending" };
const approved: SorafsAliasManifestStatusV1 = { state: "approved", epoch: 7 };
const retired: SorafsAliasManifestStatusV1 = { state: "retired", epoch: 9 };
const decision: SorafsAliasCacheDecisionV1 = "refuse";
const reason: SorafsAliasCacheReasonV1 = "ApprovedSuccessorPending";
const optionalLiveExpiry: Pick<SorafsAliasRecord, "proof_expires_in_seconds"> = {};
const attestation: Pick<SorafsAliasListResponse, "attestation"> = {
  attestation: {
    block_height: 1,
    block_hash_hex: "ab".repeat(32),
    chain_id: "fixture-chain",
  },
};

const retiredPendingEpoch: SorafsAliasManifestStatusV1 = {
  state: "pending",
  // @ts-expect-error pending status carries no epoch field.
  epoch: 1,
};
const nullableLiveExpiry: Pick<SorafsAliasRecord, "proof_expires_in_seconds"> = {
  // @ts-expect-error the conditional field is omitted, never null.
  proof_expires_in_seconds: null,
};
const nullableAttestation: Pick<SorafsAliasListResponse, "attestation"> = {
  // @ts-expect-error alias pages always carry committed-chain attestation.
  attestation: null,
};
// @ts-expect-error reason literals retain exact first-release casing.
const normalizedReason: SorafsAliasCacheReasonV1 = "approved_successor_pending";
// @ts-expect-error the cache decision union is closed.
const retiredDecision: SorafsAliasCacheDecisionV1 = "allow";

void [
  pending,
  approved,
  retired,
  decision,
  reason,
  optionalLiveExpiry,
  attestation,
  retiredPendingEpoch,
  nullableLiveExpiry,
  nullableAttestation,
  normalizedReason,
  retiredDecision,
];
