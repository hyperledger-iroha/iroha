import {
  type OfflineOperationStatus,
  type OfflineRedeemRequestJson,
  type OfflineTopUpRequestJson,
  ToriiBrowserClient,
  ToriiClient,
} from "../../index.js";

const operationId = Array<number>(32).fill(0x11);
const authorization = { operation_id: operationId };

const topUp: OfflineTopUpRequestJson = {
  asset: "xor##alice",
  amount: { atomic_units: 9_007_199_254_740_993n, scale: 4 },
  current_note: { version: 2 },
  record_bundle: { version: 2 },
  pallas_open_envelopes_archive: [1, 2, 3],
  artifact_generation: "generation-1",
  operation_id: operationId,
  authorization,
};

const redeem: OfflineRedeemRequestJson = {
  bundle: { version: 2 },
  recipient: "alice",
  amount: { atomic_units: 7n, scale: 0 },
  redeem_proof: { version: 1 },
  redemption: { version: 2 },
  lineage_verifier_record: { id: "lineage-vk" },
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
  void evaluatedHeight;
  void evaluatedHash;
  if (readiness.ready) {
    const accepted = await nodeClient.submitOfflineTopUp(topUp);
    const submittedAt: number | bigint = accepted.submitted_at_ms;
    void submittedAt;
    const status: OfflineOperationStatus = await nodeClient.getOfflineOperationStatus(
      accepted.operation_id,
    );
    if (status.state === "applied" && status.value.result.kind === "top_up") {
      status.value.result.result.anchor;
    }
  }
};

const browserFlow = async (): Promise<void> => {
  const accepted = await browserClient.submitOfflineRedeem(redeem);
  const status = await browserClient.getOfflineOperationStatus(accepted.operation_id);
  if (status.state === "rejected") {
    status.value.error.code;
  }
};

void nodeFlow;
void browserFlow;
