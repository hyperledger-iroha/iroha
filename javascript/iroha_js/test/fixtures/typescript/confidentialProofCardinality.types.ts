import { ConfidentialProver, ConfidentialProverError, type ConfidentialProof } from "../../../index.js";
declare const wallet: ConfidentialProver;
type Transfer = Parameters<ConfidentialProver["proveTransfer"]>[0];
declare const note: Transfer["inputs"][0];
declare const output: Transfer["outputs"][0];
declare const request: Transfer;
wallet.proveTransfer({ ...request, inputs: [note], outputs: [output] });
wallet.proveTransfer({ ...request, inputs: [note, note], outputs: [output, output] });
// @ts-expect-error There are two input slots.
wallet.proveTransfer({ ...request, inputs: [note, note, note] });
// @ts-expect-error A transfer needs an actual output.
wallet.proveTransfer({ ...request, outputs: [] });
// @ts-expect-error There are two output slots.
wallet.proveTransfer({ ...request, outputs: [output, output, output] });
// @ts-expect-error Redemption also requires an actual input.
wallet.proveRedemption({ ...request, inputs: [], publicAmount: 7n });
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

import { computeConfidentialRoot } from "../../../index.js";
const pendingRoot: Promise<Buffer> = computeConfidentialRoot({ commitments: [] });
void pendingRoot;
// @ts-expect-error Root computation uses the canonical fixed depth.
computeConfidentialRoot({ commitments: [], depth: 1 });
// @ts-expect-error Circuit and verifying-key selection belongs to the prover.
wallet.proveTransfer({ ...request, verifyingKey: {} });
// @ts-expect-error Caller-selected proof builders are absent from the first-release API.
const retiredBuilder: typeof import("../../../index.js")["buildConfidentialTransferProofV2"] = undefined;
void retiredBuilder;

import { confidentialChangeToInput, defaultConfidentialDiversifier } from "../../../index.js";
const changeInput: Transfer["inputs"][0] = confidentialChangeToInput({ amount: 2n, rhoHex: "" }, 1);
const defaultDiversifier: Buffer = defaultConfidentialDiversifier();
void changeInput; void defaultDiversifier;
// @ts-expect-error Change ownership uses the native default, not a caller-selected diversifier.
confidentialChangeToInput({ amount: 2n, rhoHex: "", diversifierHex: "" }, 1);
// @ts-expect-error Leaf indices have the same numeric type as actual input notes.
confidentialChangeToInput({ amount: 2n, rhoHex: "" }, 1n);
