import type {
  ToriiBrowserClient,
  ToriiBrowserExplorerInstruction,
  ToriiBrowserExplorerInstructionBox,
  ToriiBrowserExplorerTransactionDetail,
  ToriiBrowserExplorerTransactionRejection,
} from "../../../index.js";

const box: ToriiBrowserExplorerInstructionBox = {
  wire_id: "iroha.set_key_value",
  framed_sha256: "9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a",
  instruction: "AQIDBA==",
};
const rejection: ToriiBrowserExplorerTransactionRejection = {
  reason: "AQ==",
  message: "validation failed",
};
// @ts-expect-error native frames do not return a parallel encoded hex field.
const encoded: Pick<ToriiBrowserExplorerInstructionBox, "encoded"> = { encoded: "0x01" };
// @ts-expect-error native frames do not return a parallel JSON instruction tree.
const json: Pick<ToriiBrowserExplorerInstructionBox, "json"> = { json: {} };
// @ts-expect-error every box carries the original native InstructionBox frame.
const missing: ToriiBrowserExplorerInstructionBox = { wire_id: "iroha.log", framed_sha256: "hash" };
// @ts-expect-error rejection reasons use the native reason frame.
const oldReason: ToriiBrowserExplorerTransactionRejection = { encoded: "0x01", json: {}, message: "failed" };

declare const client: ToriiBrowserClient;
const instruction: Promise<ToriiBrowserExplorerInstruction> = client.getExplorerInstruction("hash", 0);
const transaction: Promise<ToriiBrowserExplorerTransactionDetail> = client.getExplorerTransaction("hash");
void [box, rejection, encoded, json, missing, oldReason, instruction, transaction];
