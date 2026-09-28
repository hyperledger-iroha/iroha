import {
  buildConfidentialTransferProofV2, buildConfidentialUnshieldProofV2, buildConfidentialUnshieldProofV3,
} from "../../../index.js";

type Transfer = Parameters<typeof buildConfidentialTransferProofV2>[0];
type Unshield = Parameters<typeof buildConfidentialUnshieldProofV3>[0];
declare const request: Transfer;
declare const note: Transfer["inputs"][0];
declare const output: Transfer["outputs"][0];
declare const unshield: Unshield;
buildConfidentialTransferProofV2({ ...request, inputs: [note], outputs: [output] });
buildConfidentialTransferProofV2({ ...request, inputs: [note, note], outputs: [output, output] });
buildConfidentialUnshieldProofV2({ ...unshield, inputs: [note] });
buildConfidentialUnshieldProofV3({ ...unshield, inputs: [note], outputs: [] });
buildConfidentialUnshieldProofV3({ ...unshield, inputs: [note, note], outputs: [output] });
// @ts-expect-error A transfer cannot consume zero actual inputs.
buildConfidentialTransferProofV2({ ...request, inputs: [] });
// @ts-expect-error This circuit has two input slots.
buildConfidentialTransferProofV2({ ...request, inputs: [note, note, note] });
// @ts-expect-error A transfer requires an actual output.
buildConfidentialTransferProofV2({ ...request, outputs: [] });
// @ts-expect-error This circuit has two output slots.
buildConfidentialTransferProofV2({ ...request, outputs: [output, output, output] });
// @ts-expect-error Unshield also requires an actual input.
buildConfidentialUnshieldProofV2({ ...unshield, inputs: [] });
// @ts-expect-error V3 unshield has at most one change output.
buildConfidentialUnshieldProofV3({ ...unshield, outputs: [output, output] });

import { ConfidentialProver, ConfidentialProverError, type ConfidentialProof } from "../../../index.js";
declare const wallet: ConfidentialProver;
const pendingProof: Promise<ConfidentialProof> = wallet.proveTransfer({ inputs: [note], outputs: [output], treeCommitments: [], rootHex: "" });
// @ts-expect-error Proving is asynchronous and cannot be treated as an immediate proof.
const immediateProof: ConfidentialProof = pendingProof;
void immediateProof;
wallet.proveRedemption({ inputs: [note], publicAmount: 7n, treeCommitments: [], rootHex: "" });
wallet.proveRedemption({ inputs: [note], publicAmount: 5n, change: { amount: 2n, rhoHex: "" }, treeCommitments: [], rootHex: "" });
wallet.dispose();
declare const walletError: ConfidentialProverError;
const failureCode: "INVALID_INPUT" | "NATIVE_UNAVAILABLE" | "PROVING_FAILED" | "DISPOSED" = walletError.code;
void failureCode;
// @ts-expect-error Canonical wallet proving accepts exactly one or two inputs.
wallet.proveTransfer({ inputs: [], outputs: [output], treeCommitments: [], rootHex: "" });
// @ts-expect-error The private change is a single note, not a circuit-specific output vector.
wallet.proveRedemption({ inputs: [note], publicAmount: 5n, change: [output], treeCommitments: [], rootHex: "" });
