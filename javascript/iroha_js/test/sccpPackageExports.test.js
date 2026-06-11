import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import * as rootExports from "../dist/index.js";
import * as sccpExports from "../dist/sccp.js";
import {
  SCCP_DOMAIN_TON,
  SCCP_SOURCE_STATE_MAX_PROOF_BYTES,
  SolanaSccpSourceStateProver,
  TonSccpSourceStateProver,
  buildEvmSccpBridgeProofSubmitPayload,
  buildSolanaSccpFullLightClientAuditProofRequests,
  buildTonShardStateProofRequest,
  buildTonSccpFullLightClientAuditProofRequests,
  buildTronSccpBridgeProofSubmitPayload,
  canonicalSccpMessageProofBundleBytes,
  canonicalSccpMerkleProofBytes,
  canonicalSccpPayloadEnvelopeBytes,
  evmSccpDestinationBindingHash,
  tronSccpDestinationBindingHash,
  wrapTonSccpSourceStateVerificationProof,
} from "../dist/index.js";

function declarationInterface(declarations, name) {
  const match = declarations.match(
    new RegExp(`export interface ${name}(?: extends [^{]+)? \\{[\\s\\S]*?\\n\\}`),
  );
  assert.ok(match, `missing declaration interface ${name}`);
  return match[0];
}

function declarationFunction(declarations, name) {
  const match = declarations.match(
    new RegExp(`export function ${name}\\([\\s\\S]*?\\): [^;]+;`),
  );
  assert.ok(match, `missing declaration function ${name}`);
  return match[0];
}

function assertDomainInputFields(interfaceDeclaration, fields) {
  for (const field of fields) {
    assert.match(
      interfaceDeclaration,
      new RegExp(`${field}\\?: SccpDomainIdInput;`, "u"),
      field,
    );
  }
}

function assertVersionInputFields(interfaceDeclaration, fields = ["version"]) {
  for (const field of fields) {
    assert.match(
      interfaceDeclaration,
      new RegExp(`${field}\\?: SccpVersionInput;`, "u"),
      field,
    );
  }
}

test("published package root re-exports every SCCP subpath symbol", () => {
  const sccpExportNames = Object.keys(sccpExports).sort();
  const missing = sccpExportNames.filter((name) => !(name in rootExports));

  assert.deepEqual(missing, []);
  for (const name of sccpExportNames) {
    assert.equal(rootExports[name], sccpExports[name], name);
  }
});

test("published TypeScript declarations cover every SCCP runtime export", () => {
  const declarations = fs.readFileSync(
    new URL("../index.d.ts", import.meta.url),
    "utf8",
  );
  const declaredNames = new Set();
  const declarationPattern =
    /export\s+(?:declare\s+)?(?:const|function|class|interface|type)\s+([A-Za-z_$][\w$]*)/gu;
  for (const match of declarations.matchAll(declarationPattern)) {
    declaredNames.add(match[1]);
  }

  const missing = Object.keys(sccpExports)
    .sort()
    .filter((name) => !declaredNames.has(name));

  assert.deepEqual(missing, []);
});

test("published SCCP package excludes the removed legacy lane", () => {
  const removedLaneToken = ["sub", "strate"].join("");
  const declarations = fs.readFileSync(
    new URL("../index.d.ts", import.meta.url),
    "utf8",
  );
  const exportNames = [
    ...Object.keys(rootExports),
    ...Object.keys(sccpExports),
  ].sort();
  const leakedExports = exportNames.filter((name) =>
    name.toLowerCase().includes(removedLaneToken),
  );

  assert.deepEqual(leakedExports, []);
  assert.equal(declarations.toLowerCase().includes(removedLaneToken), false);
  for (const artifact of ["../dist/index.js", "../dist/sccp.js", "../dist/toriiClient.js"]) {
    const source = fs.readFileSync(new URL(artifact, import.meta.url), "utf8");
    assert.equal(source.toLowerCase().includes(removedLaneToken), false, artifact);
  }
});

test("published TypeScript declarations constrain TAIRA XOR TRON settlement defaults", () => {
  const declarations = fs.readFileSync(
    new URL("../index.d.ts", import.meta.url),
    "utf8",
  );
  const settlementFragment = declarationInterface(
    declarations,
    "TairaXorTronToTairaSettlementFragment",
  );
  assert.match(settlementFragment, /entrypoint\?: "finalize_inbound";/u);
  assert.match(settlementFragment, /route\?: "taira_tron_xor";/u);
  assert.match(settlementFragment, /route_id\?: "taira_tron_xor";/u);
  assert.match(settlementFragment, /payload\?: never;/u);
  assert.match(settlementFragment, /payload_json\?: never;/u);
  assert.match(settlementFragment, /payloadJson\?: never;/u);
  assert.match(settlementFragment, /payload_bytes\?: never;/u);
  assert.match(settlementFragment, /payloadBytes\?: never;/u);

  const sourcePackageInput = declarationInterface(
    declarations,
    "TairaXorTronToTairaSourceProofPackageInput",
  );
  assert.match(
    sourcePackageInput,
    /settlementDefaults\?: TairaXorTronToTairaSettlementFragment;/u,
  );
  assert.match(
    sourcePackageInput,
    /settlement_defaults\?: TairaXorTronToTairaSettlementFragment;/u,
  );
});

test("published TypeScript declarations expose SCCP domain id inputs", () => {
  const declarations = fs.readFileSync(
    new URL("../index.d.ts", import.meta.url),
    "utf8",
  );

  assert.match(
    declarations,
    /export type SccpDomainIdInput = number \| string \| bigint;/u,
  );
  assert.match(
    declarations,
    /export type SccpDestinationBindingDomainInput =\n  \| SccpDomainIdInput\n  \| \{ targetDomain\?: SccpDomainIdInput; target_domain\?: SccpDomainIdInput; domain\?: SccpDomainIdInput \};/u,
  );

  assertDomainInputFields(
    declarationInterface(declarations, "SolanaSccpWitnessInput"),
    ["targetDomain", "target_domain"],
  );
  assertDomainInputFields(
    declarationInterface(declarations, "SolanaSccpBankForkInput"),
    ["sourceDomain", "source_domain"],
  );
  assertDomainInputFields(
    declarationInterface(declarations, "SccpSourceAdapterDeploymentBindingInput"),
    ["sourceDomain", "source_domain", "targetDomain", "target_domain"],
  );
  assertDomainInputFields(
    declarationInterface(declarations, "EvmSccpDestinationBindingInput"),
    ["sourceDomain", "source_domain", "targetDomain", "target_domain"],
  );
  assertDomainInputFields(
    declarationInterface(declarations, "TronSccpDestinationBindingInput"),
    ["sourceDomain", "source_domain", "targetDomain", "target_domain"],
  );
  assertDomainInputFields(
    declarationInterface(declarations, "SccpSourceVerifierMaterialInput"),
    ["sourceDomain", "source_domain", "targetDomain", "target_domain"],
  );
  assertDomainInputFields(
    declarationInterface(declarations, "TonSccpManifestInput"),
    ["localDomain", "local_domain", "counterpartyDomain", "counterparty_domain"],
  );
  assertDomainInputFields(
    declarationInterface(declarations, "TonSccpProofRequestInput"),
    ["sourceDomain", "source_domain"],
  );
  assertDomainInputFields(
    declarationInterface(declarations, "EvmSccpReceiptProofInput"),
    ["sourceDomain", "source_domain"],
  );
  assertDomainInputFields(
    declarationInterface(declarations, "BscSccpReceiptProofInput"),
    ["sourceDomain", "source_domain"],
  );
  const verifierVkHash = declarationFunction(declarations, "sccpSourceAdapterVerifierVkHash");
  assert.match(verifierVkHash, /input: SccpDomainIdInput \| \{/u);
  assert.match(verifierVkHash, /target_domain\?: SccpDomainIdInput/u);
  assert.doesNotMatch(
    declarations,
    /\b(?:sourceDomain|source_domain|targetDomain|target_domain|localDomain|local_domain|counterpartyDomain|counterparty_domain)\?: number;/u,
  );
});

test("published TypeScript declarations expose SCCP v1 version inputs", () => {
  const declarations = fs.readFileSync(
    new URL("../index.d.ts", import.meta.url),
    "utf8",
  );
  const sccpDeclarations = declarations.slice(
    declarations.indexOf("export interface SccpBurnPayload"),
    declarations.indexOf("export type SccpWitnessProviderFn"),
  );

  assert.match(
    declarations,
    /export type SccpVersionInput = 1 \| "1" \| 1n;/u,
  );
  assert.match(
    declarations,
    /export type SccpSourceStateOptionalVersionResultMetadata[\s\S]*version\?: SccpVersionInput;[\s\S]*proofVersion: SccpVersionInput;[\s\S]*proof_version: SccpVersionInput;/u,
  );
  assert.match(
    declarations,
    /export type SolanaSccpSourceStateVerificationProof =[\s\S]*SccpSourceStateProofCapsuleMetadata/u,
  );
  assert.match(
    declarations,
    /export type TonSccpSourceStateVerificationProof =[\s\S]*SccpSourceStateProofCapsuleMetadata/u,
  );
  assertVersionInputFields(declarationInterface(declarations, "EvmSccpDestinationBindingInput"));
  assertVersionInputFields(declarationInterface(declarations, "TronSccpDestinationBindingInput"));
  assertVersionInputFields(declarationInterface(declarations, "TonMasterchainConfigLeafInput"));
  assertVersionInputFields(declarationInterface(declarations, "TonValidatorSignatureProofInput"));
  assertVersionInputFields(declarationInterface(declarations, "EthBeaconSyncCommitteeProofInput"));
  assertVersionInputFields(declarationInterface(declarations, "BscValidatorStorageProofInput"));
  assertVersionInputFields(declarationInterface(declarations, "TronSolidBlockMessageInput"));
  assertVersionInputFields(declarationInterface(declarations, "TronWitnessSealInput"));
  assertVersionInputFields(declarationInterface(declarations, "SccpMessageTransparentPublicInputsInput"));
  assert.match(
    declarationInterface(declarations, "TonSccpManifestInput"),
    /version: SccpVersionInput;/u,
  );
  assert.doesNotMatch(
    sccpDeclarations,
    /\bversion\?: (?:number|string \| number \| bigint|1|1 \| "1" \| 1n);/u,
  );
});

test("published TypeScript declarations expose BSC Parlia commit inputs", () => {
  const declarations = fs.readFileSync(
    new URL("../index.d.ts", import.meta.url),
    "utf8",
  );
  const commitMessageInput = declarationInterface(declarations, "BscCommitMessageInput");
  const commitSealInput = declarationInterface(declarations, "BscCommitSealInput");

  assert.match(commitMessageInput, /version\?: SccpVersionInput;/u);
  assert.match(commitMessageInput, /sourceDomain\?: SccpDomainIdInput;/u);
  assert.match(commitMessageInput, /source_domain\?: SccpDomainIdInput;/u);
  assert.match(commitMessageInput, /validatorEpoch\?: string \| number \| bigint;/u);
  assert.match(commitMessageInput, /validator_epoch\?: string \| number \| bigint;/u);
  assert.match(commitMessageInput, /validatorSetHash\?: string;/u);
  assert.match(commitMessageInput, /validator_set_hash\?: string;/u);
  assert.match(commitSealInput, /version\?: SccpVersionInput;/u);
  assert.match(commitSealInput, /commitMessageHash\?: string;/u);
  assert.match(commitSealInput, /commit_message_hash\?: string;/u);
  assert.match(commitSealInput, /validatorPublicKeys\?: ReadonlyArray<BinaryLike>;/u);
  assert.match(commitSealInput, /validator_public_keys\?: ReadonlyArray<BinaryLike>;/u);
  assert.match(commitSealInput, /signersBitmap\?: BinaryLike;/u);
  assert.match(commitSealInput, /signers_bitmap\?: BinaryLike;/u);
  assert.match(commitSealInput, /validatorSetHash\?: string;/u);
  assert.match(commitSealInput, /validator_set_hash\?: string;/u);
  assert.match(
    declarations,
    /export function canonicalBscCommitMessageBytes\(input: BscCommitMessageInput\): Uint8Array;/u,
  );
  assert.match(
    declarations,
    /export function bscCommitMessageHash\(input: BscCommitMessageInput\): string;/u,
  );
  assert.match(
    declarations,
    /export function canonicalBscCommitSealBytes\(input: BscCommitSealInput\): Uint8Array;/u,
  );
  assert.match(
    declarations,
    /export function bscCommitSealHash\(input: BscCommitSealInput\): string;/u,
  );
});

const samplePackageRootTonShardStateSourceStateInput = () => ({
  version: 1,
  sourceDomain: SCCP_DOMAIN_TON,
  masterchainSeqno: 19n,
  masterchainWorkchainId: -1,
  masterchainShard: 0x8000000000000000n,
  masterchainBlockHash: `0x${"aa".repeat(32)}`,
  masterchainFileHash: `0x${"a5".repeat(32)}`,
  validatorSetHash: `0x${"b1".repeat(32)}`,
  masterchainConfigRoot:
    "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af",
  masterchainConfigProofHash: `0x${"b2".repeat(32)}`,
  shardWorkchainId: 0,
  shardShard: 0x8000000000000000n,
  shardSeqno: 7n,
  shardBlockHash: `0x${"bb".repeat(32)}`,
  shardFileHash: `0x${"bc".repeat(32)}`,
  shardStateRoot:
    "0x12a960855fea2f529c336d7325b1cca784f0f0b1a52ae149d02d046a2499e270",
  transactionRoot:
    "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
  transactionLt: 7n,
  shardStateDictionaryRoot:
    "0x049a63ecefc78dc0cd468ebf47e0385807d790a2ca8e0dca5cbbeb0714567fd3",
  shardStateDictionaryKeyBitLen: 256,
  shardStateDictionaryKey: Uint8Array.from([17, ...Array(31).fill(0)]),
  masterchainSignatureHash: `0x${"c1".repeat(32)}`,
  shardProofHash:
    "0x32d8b496320e6a1ce5ccf671f2bd6f0d09cb53afed8c123b86cb9327b77c88cf",
  shardStateProofBoc: Buffer.from(
    "b5ee9c720101060100aa00035b9023afe2ffffff110000000000000000000000000000000007000000010000000b000000000000000c000000122001020500000101c00301d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419000000000000000780400000000",
    "hex",
  ),
  shardStateDictionaryProofBoc: Buffer.from(
    "b5ee9c72010103010073000101c00101d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e41900000000000000078020000",
    "hex",
  ),
  configDictionaryProofBoc: Buffer.from(
    "b5ee9c72010106010091000101c00101117fffffff80000008a002012b120000000100000002000200020000000000000003c00302087fff00000405005b14e3a049e28444444444444444444444444444444444444444444444444444444444444444400000000000000060005b14e3a049e288888888888888888888888888888888888888888888888888888888888888000000000000000a0",
    "hex",
  ),
  validatorSetTransitionProofs: [],
  sourceStateVerifierHash: `0x${"d4".repeat(32)}`,
  sourceTrustAnchorHash: `0x${"d5".repeat(32)}`,
  consensusVerifierHash: `0x${"d6".repeat(32)}`,
  messageInclusionVerifierHash: `0x${"d7".repeat(32)}`,
  finalityPolicyHash: `0x${"d8".repeat(32)}`,
});

test("published package root enforces TON source-state proof cap", async () => {
  const request = buildTonShardStateProofRequest(
    samplePackageRootTonShardStateSourceStateInput(),
  );
  const oversizedTonPackageRootSourceStateProofBytes = new Uint8Array(
    SCCP_SOURCE_STATE_MAX_PROOF_BYTES + 1,
  ).fill(1);

  assert.throws(
    () =>
      wrapTonSccpSourceStateVerificationProof(
        oversizedTonPackageRootSourceStateProofBytes,
        request,
      ),
    /proofBytes must be at most/u,
  );

  const oversizedTonPackageRootCallbackProver = new TonSccpSourceStateProver({
    prove() {
      return oversizedTonPackageRootSourceStateProofBytes;
    },
  });
  await assert.rejects(
    () =>
      oversizedTonPackageRootCallbackProver.proveShardState(
        samplePackageRootTonShardStateSourceStateInput(),
      ),
    /proofBytes must be at most/u,
  );
});

test("published package root exports SCCP destination binding helpers", () => {
  const evmHash = evmSccpDestinationBindingHash({
    targetDomain: 1,
    networkId: `0x${"33".repeat(32)}`,
    verifierAddress: `0x${"11".repeat(20)}`,
    bridgeAddress: `0x${"22".repeat(20)}`,
    verifierCodeHash: `0x${"bb".repeat(32)}`,
    verifierKeyHash: `0x${"cc".repeat(32)}`,
  });
  const tronHash = tronSccpDestinationBindingHash({
    networkId: `0x${"33".repeat(32)}`,
    verifierAddress: "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
    verifierCodeHash: `0x${"bb".repeat(32)}`,
    verifierKeyHash: `0x${"cc".repeat(32)}`,
  });

  assert.equal(
    evmHash,
    "0x3ad95ac3e5bc2892f768aae40a3b7ba673d561858b7d1318fbb9f6eba83207bf",
  );
  assert.equal(
    tronHash,
    "0x17c953ad5b8c9a2b6f7102aca993fa7c427d018505cf4f58fac35ea454caba7f",
  );
  assert.equal(typeof SolanaSccpSourceStateProver, "function");
  assert.equal(typeof TonSccpSourceStateProver, "function");
  assert.equal(typeof buildSolanaSccpFullLightClientAuditProofRequests, "function");
  assert.equal(typeof buildTonSccpFullLightClientAuditProofRequests, "function");
  assert.equal(typeof buildEvmSccpBridgeProofSubmitPayload, "function");
  assert.equal(typeof buildTronSccpBridgeProofSubmitPayload, "function");
  assert.equal(typeof canonicalSccpPayloadEnvelopeBytes, "function");
  assert.equal(typeof canonicalSccpMerkleProofBytes, "function");
  assert.equal(typeof canonicalSccpMessageProofBundleBytes, "function");
});
