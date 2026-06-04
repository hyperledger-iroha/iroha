import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import * as rootExports from "../dist/index.js";
import * as sccpExports from "../dist/sccp.js";
import {
  SolanaSccpSourceStateProver,
  TonSccpSourceStateProver,
  buildEvmSccpBridgeProofSubmitPayload,
  buildSolanaSccpFullLightClientAuditProofRequests,
  buildSubstrateSccpSubmission,
  buildTonSccpFullLightClientAuditProofRequests,
  buildTronSccpBridgeProofSubmitPayload,
  canonicalSccpMessageProofBundleBytes,
  canonicalSccpMerkleProofBytes,
  canonicalSccpPayloadEnvelopeBytes,
  evmSccpDestinationBindingHash,
  tronSccpDestinationBindingHash,
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
  assertDomainInputFields(
    declarationInterface(declarations, "SubstrateSccpProofRequestInput"),
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
  assertVersionInputFields(declarationInterface(declarations, "SubstrateGrandpaJustificationProofInput"));
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
  assert.equal(typeof buildSubstrateSccpSubmission, "function");
  assert.equal(typeof buildTonSccpFullLightClientAuditProofRequests, "function");
  assert.equal(typeof buildEvmSccpBridgeProofSubmitPayload, "function");
  assert.equal(typeof buildTronSccpBridgeProofSubmitPayload, "function");
  assert.equal(typeof canonicalSccpPayloadEnvelopeBytes, "function");
  assert.equal(typeof canonicalSccpMerkleProofBytes, "function");
  assert.equal(typeof canonicalSccpMessageProofBundleBytes, "function");
});
