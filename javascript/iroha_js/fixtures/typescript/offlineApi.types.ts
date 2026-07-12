import {
  type OfflineAuthorizationJson,
  type OfflineFixed32Bytes,
  type OfflineLineageWitnessJson,
  type OfflineOperationStatus,
  type OfflineProofAttachmentJson,
  type OfflineRedeemRequestJson,
  type OfflineRecursiveSpendBundleJson,
  type OfflineRedeemChangeJson,
  type OfflineRedemptionIntentJson,
  type OfflineSpendableNoteJson,
  type OfflineTopUpRequestJson,
  type OfflineVerifyingKeyRecordJson,
  type OfflineVerifiedFoldRecordBundleJson,
  type ToriiOfflineActiveTopUpShieldVerifier,
  ToriiBrowserClient,
  ToriiClient,
} from "../../index.js";

declare const operationId: OfflineFixed32Bytes;
declare const fixed32: OfflineFixed32Bytes;
declare const currentNote: OfflineSpendableNoteJson;
declare const recordBundle: OfflineVerifiedFoldRecordBundleJson;
declare const recursiveBundle: OfflineRecursiveSpendBundleJson;
declare const redeemProof: OfflineProofAttachmentJson;
declare const redemption: OfflineRedemptionIntentJson;
declare const lineageWitness: OfflineLineageWitnessJson;
declare const lineageVerifierRecord: OfflineVerifyingKeyRecordJson;
declare const offlineChange: OfflineRedeemChangeJson;

const authorization: OfflineAuthorizationJson = {
  authority: "alice",
  device_id: "wallet-1",
  operation_id: operationId,
  issued_at_ms: 1n,
  expires_at_ms: 2n,
  nonce: fixed32,
  payload_digest: fixed32,
  app_attest_evidence_sha256: null,
  app_attest_evidence: null,
  signature: "AA",
};

const topUp: OfflineTopUpRequestJson = {
  asset: "xor##alice",
  amount: { atomic_units: 9_007_199_254_740_993n, scale: 4 },
  current_note: currentNote,
  record_bundle: recordBundle,
  pallas_open_envelopes_archive: [1, 2, 3],
  artifact_generation: "generation-1",
  operation_id: operationId,
  authorization,
};

const redeem: OfflineRedeemRequestJson = {
  bundle: recursiveBundle,
  recipient: "alice",
  amount: { atomic_units: 7n, scale: 0 },
  redeem_proof: redeemProof,
  redemption,
  lineage_witness: lineageWitness,
  lineage_verifier_record: lineageVerifierRecord,
  offline_change: offlineChange,
  block_height: 42n,
  operation_id: operationId,
  authorization,
};

declare const nodeClient: ToriiClient;
declare const browserClient: ToriiBrowserClient;

const nodeFlow = async (): Promise<void> => {
  const readiness = await nodeClient.getOfflineReadiness("xor#sora");
  const evaluatedHeight: number | bigint = readiness.evaluated_block_height;
  const evaluatedHash: string = readiness.evaluated_block_hash;
  const topUpShieldVerifier: ToriiOfflineActiveTopUpShieldVerifier | null =
    readiness.active_topup_shield_verifier;
  void evaluatedHeight;
  void evaluatedHash;
  void topUpShieldVerifier;
  if (readiness.ready) {
    const accepted = await nodeClient.submitOfflineTopUp(topUp);
    const submittedAt: number | bigint = accepted.submitted_at_ms;
    void submittedAt;
    const status: OfflineOperationStatus = await nodeClient.getOfflineOperationStatus(
      accepted.operation_id,
    );
    if (status.state === "applied" && status.value.result.kind === "top_up") {
      const digest: OfflineFixed32Bytes = status.value.result.result.anchor.anchor_digest;
      void digest;
    }
  }
};

// @ts-expect-error the first-release Offline contract caps asset scales at 28.
const invalidScale: OfflineTopUpRequestJson = { ...topUp, amount: { atomic_units: 1, scale: 29 } };
void invalidScale;

// @ts-expect-error whole-payload wrappers are not part of the direct request DTO.
const wrappedTopUp: OfflineTopUpRequestJson = { ...topUp, request_archive: "AA==" };
void wrappedTopUp;

const browserFlow = async (): Promise<void> => {
  const accepted = await browserClient.submitOfflineRedeem(redeem);
  const status = await browserClient.getOfflineOperationStatus(accepted.operation_id);
  if (status.state === "rejected") {
    status.value.error.code;
  }
};

void nodeFlow;
void browserFlow;
