import assert from "node:assert/strict";
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import {
  chmod,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { test } from "node:test";
import {
  BSC_FULL_SCCP_CIRCUIT_PROFILE,
  BSC_GROTH16_CIRCUIT_SECURITY_ATTESTATION_SCHEMA,
  BSC_GROTH16_ATTESTATION_REQUEST_PACKAGE_SCHEMA,
  BSC_GROTH16_ATTESTATION_HANDOFF_SCHEMA,
  BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA,
  BSC_GROTH16_CIRCUIT_SECURITY_AUDIT_EVIDENCE_SCHEMA,
  BSC_GROTH16_EVIDENCE_TEMPLATE_PACKAGE_SCHEMA,
  BSC_GROTH16_MATERIAL_MANIFEST_SCHEMA,
  BSC_GROTH16_PROOF_SELF_TEST_SCHEMA,
  BSC_GROTH16_PUBLIC_SIGNAL_NAMES,
  BSC_GROTH16_REPRODUCIBLE_BUILD_ATTESTATION_SCHEMA,
  BSC_GROTH16_REPRODUCIBLE_BUILD_TRANSCRIPT_SCHEMA,
  BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA,
  BSC_GROTH16_SEMANTIC_REVIEW_EVIDENCE_SCHEMA,
  BSC_GROTH16_TRANSCRIPT_TEMPLATE_PACKAGE_SCHEMA,
  BSC_GROTH16_TRUSTED_SETUP_ATTESTATION_SCHEMA,
  BSC_GROTH16_TRUSTED_SETUP_TRANSCRIPT_SCHEMA,
  BSC_SIGNAL_BINDING_CIRCUIT_PROFILE,
  DEFAULT_BSC_FULL_MESSAGE_CIRCUIT_SOURCE,
  auditBscGroth16AttestationStatus,
  finalizeBscGroth16Attestations,
  fingerprintBscGroth16Toolchain,
  generateBscFullMessageCircuitSource,
  generateBscGroth16Material,
  generateBscSignalBindingCircuitSource,
  inventoryBscGroth16Attestations,
  main,
  materializeBscGroth16Material,
  preflightBscGroth16Material,
  signBscGroth16AttestationRole,
  snarkjsVerificationKeyToBscVerifierMaterial,
  verifyBscGroth16AttestationHandoff,
  writeBscGroth16AttestationHandoff,
  writeBscGroth16EvidenceTemplates,
  writeBscGroth16TranscriptTemplates,
} from "./sccp_bsc_groth16_material.mjs";
import {
  BSC_MAINNET_CHAIN_ID_HEX,
  BSC_MAINNET_NETWORK_ID_HEX,
  BSC_TESTNET_CHAIN_ID_HEX,
  BSC_TESTNET_NETWORK_ID_HEX,
  bscGroth16VerifierKeyHash,
  normalizeVerifierMaterial,
} from "./sccp_bsc_taira_xor_deploy.mjs";

const sha256Hex = (bytes) =>
  `0x${createHash("sha256").update(bytes).digest("hex")}`;
const TRUSTED_SETUP_TRANSCRIPT_SCHEMA =
  BSC_GROTH16_TRUSTED_SETUP_TRANSCRIPT_SCHEMA;
const REPRODUCIBLE_BUILD_TRANSCRIPT_SCHEMA =
  BSC_GROTH16_REPRODUCIBLE_BUILD_TRANSCRIPT_SCHEMA;
const BN254_BASE_FIELD_MODULUS =
  "21888242871839275222246405745257275088696311157297823662689037894645226208583";

const TEST_ATTESTATION_SIGNER = generateKeyPairSync("ed25519");
const TEST_ATTESTATION_PUBLIC_KEY_PEM = TEST_ATTESTATION_SIGNER.publicKey.export({
  type: "spki",
  format: "pem",
});
const TEST_ATTESTATION_SIGNER_FINGERPRINT = sha256Hex(
  TEST_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "der",
  }),
);
const SECURITY_ATTESTATION_SIGNER = generateKeyPairSync("ed25519");
const SECURITY_ATTESTATION_PUBLIC_KEY_PEM =
  SECURITY_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "pem",
  });
const SECURITY_ATTESTATION_SIGNER_FINGERPRINT = sha256Hex(
  SECURITY_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "der",
  }),
);
const SETUP_ATTESTATION_SIGNER = generateKeyPairSync("ed25519");
const SETUP_ATTESTATION_PUBLIC_KEY_PEM =
  SETUP_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "pem",
  });
const SETUP_ATTESTATION_SIGNER_FINGERPRINT = sha256Hex(
  SETUP_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "der",
  }),
);
const REPRODUCIBLE_ATTESTATION_SIGNER = generateKeyPairSync("ed25519");
const REPRODUCIBLE_ATTESTATION_PUBLIC_KEY_PEM =
  REPRODUCIBLE_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "pem",
  });
const REPRODUCIBLE_ATTESTATION_SIGNER_FINGERPRINT = sha256Hex(
  REPRODUCIBLE_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "der",
  }),
);
const TRUSTED_ATTESTATION_SIGNER_FINGERPRINTS = Object.freeze([
  TEST_ATTESTATION_SIGNER_FINGERPRINT,
  SECURITY_ATTESTATION_SIGNER_FINGERPRINT,
  SETUP_ATTESTATION_SIGNER_FINGERPRINT,
  REPRODUCIBLE_ATTESTATION_SIGNER_FINGERPRINT,
]);
const UNTRUSTED_ATTESTATION_SIGNER = generateKeyPairSync("ed25519");
const UNTRUSTED_ATTESTATION_PUBLIC_KEY_PEM =
  UNTRUSTED_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "pem",
  });
const UNTRUSTED_ATTESTATION_SIGNER_FINGERPRINT = sha256Hex(
  UNTRUSTED_ATTESTATION_SIGNER.publicKey.export({
    type: "spki",
    format: "der",
  }),
);

function canonicalJson(value) {
  if (value === null) return "null";
  if (typeof value === "string") return JSON.stringify(value);
  if (typeof value === "number") return JSON.stringify(value);
  if (typeof value === "boolean") return value ? "true" : "false";
  if (Array.isArray(value)) {
    return `[${value.map((entry) => canonicalJson(entry)).join(",")}]`;
  }
  return `{${Object.keys(value)
    .sort()
    .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
    .join(",")}}`;
}

function signAttestationRecord(
  record,
  {
    privateKey = TEST_ATTESTATION_SIGNER.privateKey,
    publicKeyPem = TEST_ATTESTATION_PUBLIC_KEY_PEM,
    signerFingerprint = TEST_ATTESTATION_SIGNER_FINGERPRINT,
    algorithm = "ed25519",
    schema = BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA,
    signedPayloadSha256 = null,
  } = {},
) {
  const payload = Buffer.from(canonicalJson(record), "utf8");
  return {
    ...record,
    signature: {
      schema,
      algorithm,
      signerFingerprint,
      publicKeyPem,
      signedPayloadSha256: signedPayloadSha256 ?? sha256Hex(payload),
      signature: sign(null, payload, privateKey).toString("base64"),
    },
  };
}

async function writePrivateKeyPem(pathName, keyPair = TEST_ATTESTATION_SIGNER) {
  await writeFile(
    pathName,
    keyPair.privateKey.export({ type: "pkcs8", format: "pem" }),
    { mode: 0o600 },
  );
  return pathName;
}

const trustedSignerOption = () => ({
  "trusted-attestation-signer": TRUSTED_ATTESTATION_SIGNER_FINGERPRINTS.join(","),
});

const defaultAttestationSigning = () => ({
  semantic: {
    privateKey: TEST_ATTESTATION_SIGNER.privateKey,
    publicKeyPem: TEST_ATTESTATION_PUBLIC_KEY_PEM,
    signerFingerprint: TEST_ATTESTATION_SIGNER_FINGERPRINT,
  },
  security: {
    privateKey: SECURITY_ATTESTATION_SIGNER.privateKey,
    publicKeyPem: SECURITY_ATTESTATION_PUBLIC_KEY_PEM,
    signerFingerprint: SECURITY_ATTESTATION_SIGNER_FINGERPRINT,
  },
  setup: {
    privateKey: SETUP_ATTESTATION_SIGNER.privateKey,
    publicKeyPem: SETUP_ATTESTATION_PUBLIC_KEY_PEM,
    signerFingerprint: SETUP_ATTESTATION_SIGNER_FINGERPRINT,
  },
  reproducible: {
    privateKey: REPRODUCIBLE_ATTESTATION_SIGNER.privateKey,
    publicKeyPem: REPRODUCIBLE_ATTESTATION_PUBLIC_KEY_PEM,
    signerFingerprint: REPRODUCIBLE_ATTESTATION_SIGNER_FINGERPRINT,
  },
});

const VALID_G1 = [
  "1368015179489954701390400359078579693043519447331113978918064868415326638035",
  "9918110051302171585080402603319702774565515993150576347155970296011118125764",
  "1",
];
const VALID_G1_ALT = [
  "3353031288059533942658390886683067124040920775575537747144343083137631628272",
  "19321533766552368860946552437480515441416830039777911637913418824951667761761",
  "1",
];
const SOLIDITY_G2_GENERATOR = [
  "10857046999023057135944570762232829481370756359578518086990519993285655852781",
  "11559732032986387107991004021392285783925812861821192530917403151452391805634",
  "8495653923123431417604973247489272438418190587263600148770280649306958101930",
  "4082367875863433681332203403145435568316851327593401208105741076214120093531",
];
const SNARKJS_G2_GENERATOR = [
  [SOLIDITY_G2_GENERATOR[0], SOLIDITY_G2_GENERATOR[1]],
  [SOLIDITY_G2_GENERATOR[2], SOLIDITY_G2_GENERATOR[3]],
  ["1", "0"],
];

function verificationKey(overrides = {}) {
  return {
    protocol: "groth16",
    curve: "bn128",
    nPublic: 9,
    vk_alpha_1: VALID_G1,
    vk_beta_2: SNARKJS_G2_GENERATOR,
    vk_gamma_2: SNARKJS_G2_GENERATOR,
    vk_delta_2: SNARKJS_G2_GENERATOR,
    IC: Array.from({ length: 10 }, (_, index) =>
      index % 2 === 0 ? VALID_G1 : VALID_G1_ALT,
    ),
    ...overrides,
  };
}

function u32le(value) {
  const out = Buffer.alloc(4);
  out.writeUInt32LE(value, 0);
  return out;
}

function u64le(value) {
  const out = Buffer.alloc(8);
  out.writeUInt32LE(value >>> 0, 0);
  out.writeUInt32LE(Math.floor(value / 0x100000000), 4);
  return out;
}

function snarkjsSectionedBytes(magic, sections, version = 1) {
  const parts = [Buffer.from(magic, "ascii"), u32le(version), u32le(sections.length)];
  for (const [sectionId, payload] of sections) {
    parts.push(u32le(sectionId), u64le(payload.length));
  }
  for (const [, payload] of sections) {
    parts.push(payload);
  }
  return Buffer.concat(parts);
}

function snarkjsMaterialBytes(magic, sectionCount = 3, sizeBytes = 70 * 1024) {
  const headerBytes = 12 + sectionCount * 12;
  const payloadBytes = sizeBytes - headerBytes;
  const sectionSize = Math.floor(payloadBytes / sectionCount);
  const sections = [];
  let remaining = payloadBytes;
  for (let index = 0; index < sectionCount; index += 1) {
    const currentSize =
      index === sectionCount - 1 ? remaining : sectionSize;
    remaining -= currentSize;
    const payload = Buffer.alloc(currentSize);
    for (let cursor = 0; cursor < payload.length; cursor += 1) {
      payload[cursor] = (index * 31 + cursor * 17 + 19) & 0xff;
    }
    sections.push([index + 1, payload]);
  }
  return snarkjsSectionedBytes(magic, sections);
}

function r1csHeaderMaterialBytes({
  n8 = 32,
  nVars = 20_311_939,
  nOutputs = 0,
  nPubInputs = 9,
  nPrvInputs = 2304,
  nLabels = 20_311_939,
  nConstraints = 2_154_888,
  constraintsBytes = 70 * 1024,
  includeConstraints = true,
  includeWireMap = true,
} = {}) {
  const header = Buffer.alloc(32 + n8);
  header.writeUInt32LE(n8, 0);
  header.writeUInt32LE(nVars, 4 + n8);
  header.writeUInt32LE(nOutputs, 8 + n8);
  header.writeUInt32LE(nPubInputs, 12 + n8);
  header.writeUInt32LE(nPrvInputs, 16 + n8);
  header.writeUInt32LE(nLabels >>> 0, 20 + n8);
  header.writeUInt32LE(Math.floor(nLabels / 0x100000000), 24 + n8);
  header.writeUInt32LE(nConstraints, 28 + n8);
  const sections = [[1, header]];
  if (includeConstraints) {
    sections.push([2, Buffer.alloc(constraintsBytes, 0x42)]);
  }
  if (includeWireMap) {
    sections.push([3, Buffer.alloc(256, 0x24)]);
  }
  return snarkjsSectionedBytes("r1cs", sections);
}

function verifierKeyBytesFor(input) {
  return Buffer.from(`${JSON.stringify(input, null, 2)}\n`);
}

function attestationBase({
  schema,
  context,
  extra = {},
}) {
  return {
    schema,
    routeId: "taira_bsc_xor",
    assetKey: "xor",
    bscNetwork: "testnet",
    chain: "bsc-testnet",
    chainIdHex: BSC_TESTNET_CHAIN_ID_HEX,
    networkIdHex: BSC_TESTNET_NETWORK_ID_HEX,
    proofBackend: "evm-groth16-bn254-v1",
    proofFamily: "stark-fri-v1",
    circuitProfile: BSC_FULL_SCCP_CIRCUIT_PROFILE,
    publicInputCount: 9,
    publicSignalNames: [...BSC_GROTH16_PUBLIC_SIGNAL_NAMES],
    verifierKeyHash: context.verifierKeyHash,
    circuitSourceSha256: context.circuitSourceSha256,
    r1csSha256: context.r1csSha256,
    ...(context.powersOfTauSha256
      ? { powersOfTauSha256: context.powersOfTauSha256 }
      : {}),
    provingKeySha256: context.provingKeySha256,
    snarkjsVerificationKeySha256: context.snarkjsVerificationKeySha256,
    bscVerifierKeySha256: context.bscVerifierKeySha256,
    ...extra,
  };
}

async function writeJson(pathName, value) {
  await writeFile(pathName, `${JSON.stringify(value, null, 2)}\n`);
}

function evidenceBase({ schema, context, extra = {} }) {
  return {
    schema,
    routeId: "taira_bsc_xor",
    assetKey: "xor",
    bscNetwork: "testnet",
    chain: "bsc-testnet",
    chainIdHex: BSC_TESTNET_CHAIN_ID_HEX,
    networkIdHex: BSC_TESTNET_NETWORK_ID_HEX,
    proofBackend: "evm-groth16-bn254-v1",
    proofFamily: "stark-fri-v1",
    circuitProfile: BSC_FULL_SCCP_CIRCUIT_PROFILE,
    publicInputCount: 9,
    publicSignalNames: [...BSC_GROTH16_PUBLIC_SIGNAL_NAMES],
    verifierKeyHash: context.verifierKeyHash,
    circuitSourceSha256: context.circuitSourceSha256,
    r1csSha256: context.r1csSha256,
    powersOfTauSha256: context.powersOfTauSha256,
    provingKeySha256: context.provingKeySha256,
    snarkjsVerificationKeySha256: context.snarkjsVerificationKeySha256,
    bscVerifierKeySha256: context.bscVerifierKeySha256,
    ...extra,
  };
}

async function writeReviewEvidenceFiles(root, context, overrides = {}) {
  const semanticReport = join(root, "semantic-review-report.md");
  const circuitReport = join(root, "circuit-security-audit-report.md");
  await writeFile(
    semanticReport,
    overrides.semanticReportText ??
      "Independent semantic review confirms SCCP BSC full-message constraints.\n",
  );
  await writeFile(
    circuitReport,
    overrides.circuitReportText ??
      "Independent circuit security audit confirms production Groth16 readiness.\n",
  );
  const semanticReviewEvidence = join(root, "semantic-review-evidence.json");
  const circuitSecurityAuditEvidence = join(
    root,
    "circuit-security-audit-evidence.json",
  );
  await writeJson(
    semanticReviewEvidence,
    evidenceBase({
      schema: BSC_GROTH16_SEMANTIC_REVIEW_EVIDENCE_SCHEMA,
      context,
      extra: {
        reviewResult: "pass",
        fullSccpMessageSemantics: true,
        sourceFinalitySemantics: true,
        destinationBindingSemantics: true,
        publicSignalDerivationSemantics: true,
        negativeCaseCoverage: true,
        reviewerSignoffCount: 1,
        unresolvedFindings: 0,
        reviewReport: {
          path: "semantic-review-report.md",
          sha256: sha256Hex(await readFile(semanticReport)),
        },
        ...(overrides.semantic ?? {}),
      },
    }),
  );
  await writeJson(
    circuitSecurityAuditEvidence,
    evidenceBase({
      schema: BSC_GROTH16_CIRCUIT_SECURITY_AUDIT_EVIDENCE_SCHEMA,
      context,
      extra: {
        auditResult: "pass",
        approved: true,
        auditorSignoffCount: 1,
        criticalFindings: 0,
        highFindings: 0,
        unresolvedFindings: 0,
        auditReport: {
          path: "circuit-security-audit-report.md",
          sha256: sha256Hex(await readFile(circuitReport)),
        },
        ...(overrides.security ?? {}),
      },
    }),
  );
  return {
    semanticReviewEvidence,
    circuitSecurityAuditEvidence,
    semanticReport,
    circuitReport,
    args: [
      "--semantic-review-evidence",
      semanticReviewEvidence,
      "--circuit-security-audit-evidence",
      circuitSecurityAuditEvidence,
    ],
  };
}

function snarkjsSelfCheckContext(overrides = {}) {
  return {
    r1csInfoSource: "snarkjs-cli",
    r1csPublicInputCount: 9,
    r1csConstraintCount: 8192,
    zkeyVerify: true,
    zkeyVerifyResult: "ZKey Ok!",
    zkeyVerificationKeyExport: true,
    verifierKeyHashMatches: true,
    exportedVerifierKeyHash: overrides.verifierKeyHash,
    ...(overrides.selfCheck ?? {}),
  };
}

const transcriptOptions = (inputs) => ({
  ptau: inputs.ptau,
  ...(inputs.snarkjsStub ? { "snarkjs-bin": inputs.snarkjsStub } : {}),
  "trusted-setup-transcript": inputs.trustedSetupTranscript,
  "reproducible-build-transcript": inputs.reproducibleBuildTranscript,
});

function withSnarkjsSelfCheckContext(context, selfCheck = {}) {
  return {
    ...context,
    selfChecks: {
      ...(context.selfChecks ?? {}),
      snarkjs: snarkjsSelfCheckContext({
        verifierKeyHash: context.verifierKeyHash,
        selfCheck,
      }),
    },
  };
}

async function writeBoundAttestations(root, context, overrides = {}) {
  const semantic = join(root, "semantic.json");
  const security = join(root, "security.json");
  const setup = join(root, "setup.json");
  const reproducible = join(root, "reproducible.json");
  const snarkjsSelfCheck =
    context.selfChecks?.snarkjs ??
    snarkjsSelfCheckContext({ verifierKeyHash: context.verifierKeyHash });
  const signing = overrides.signing
    ? {
        semantic: overrides.signing,
        security: overrides.signing,
        setup: overrides.signing,
        reproducible: overrides.signing,
      }
    : {
        ...defaultAttestationSigning(),
        ...(overrides.signingByRole ?? {}),
      };
  await writeJson(
    semantic,
    signAttestationRecord(
      attestationBase({
        schema: BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA,
        context,
        extra: {
          semanticReviewEvidenceSchema: BSC_GROTH16_SEMANTIC_REVIEW_EVIDENCE_SCHEMA,
          semanticReviewEvidenceSha256:
            context.semanticReviewEvidenceSha256 ?? `0x${"11".repeat(32)}`,
          semanticReviewReportSha256:
            context.semanticReviewReportSha256 ?? `0x${"12".repeat(32)}`,
          fullSccpMessageSemantics: true,
          sourceFinalitySemantics: true,
          destinationBindingSemantics: true,
          publicSignalDerivationSemantics: true,
          negativeCaseCoverage: true,
          ...(overrides.semantic ?? {}),
        },
      }),
      signing.semantic,
    ),
  );
  await writeJson(
    security,
    signAttestationRecord(
      attestationBase({
        schema: BSC_GROTH16_CIRCUIT_SECURITY_ATTESTATION_SCHEMA,
        context,
        extra: {
          circuitSecurityAuditEvidenceSchema:
            BSC_GROTH16_CIRCUIT_SECURITY_AUDIT_EVIDENCE_SCHEMA,
          circuitSecurityAuditEvidenceSha256:
            context.circuitSecurityAuditEvidenceSha256 ?? `0x${"13".repeat(32)}`,
          circuitSecurityAuditReportSha256:
            context.circuitSecurityAuditReportSha256 ?? `0x${"14".repeat(32)}`,
          auditResult: "pass",
          approved: true,
          criticalFindings: 0,
          highFindings: 0,
          unresolvedFindings: 0,
          ...(overrides.security ?? {}),
        },
      }),
      signing.security,
    ),
  );
  await writeJson(
    setup,
    signAttestationRecord(
      attestationBase({
        schema: BSC_GROTH16_TRUSTED_SETUP_ATTESTATION_SCHEMA,
        context,
        extra: {
          ceremonyResult: "pass",
          localSingleContributor: false,
          minimumContributors: 3,
          toxicWasteDestroyed: true,
          contributionTranscriptSha256:
            context.trustedSetupTranscriptSha256 ?? `0x${"ab".repeat(32)}`,
          ...(overrides.setup ?? {}),
        },
      }),
      signing.setup,
    ),
  );
  await writeJson(
    reproducible,
    signAttestationRecord(
      attestationBase({
        schema: BSC_GROTH16_REPRODUCIBLE_BUILD_ATTESTATION_SCHEMA,
        context,
        extra: {
          reproducible: true,
          independentRebuilders: 2,
          buildTranscriptSha256:
            context.reproducibleBuildTranscriptSha256 ?? `0x${"cd".repeat(32)}`,
          toolchainSha256:
            context.reproducibleBuildToolchainSha256 ?? `0x${"ef".repeat(32)}`,
          r1csInfoSource: snarkjsSelfCheck.r1csInfoSource,
          r1csPublicInputCount: snarkjsSelfCheck.r1csPublicInputCount,
          r1csConstraintCount: snarkjsSelfCheck.r1csConstraintCount,
          zkeyVerify: snarkjsSelfCheck.zkeyVerify,
          zkeyVerifyResult: snarkjsSelfCheck.zkeyVerifyResult,
          zkeyVerificationKeyExport: snarkjsSelfCheck.zkeyVerificationKeyExport,
          verifierKeyHashMatches: snarkjsSelfCheck.verifierKeyHashMatches,
          exportedVerifierKeyHash: snarkjsSelfCheck.exportedVerifierKeyHash,
          ...(overrides.reproducible ?? {}),
        },
      }),
      signing.reproducible,
    ),
  );
  return { semantic, security, setup, reproducible };
}

async function writeAttestationsFromRequest(root, requestPath, overrides = {}) {
  const request = JSON.parse(await readFile(requestPath, "utf8"));
  const outDir = join(root, "request-attestations");
  await mkdir(outDir, { recursive: true });
  const signing = {
    ...defaultAttestationSigning(),
    ...(overrides.signingByRole ?? {}),
  };
  const rows = [
    ["semantic", "semanticSccpCircuit", signing.semantic],
    ["security", "circuitSecurity", signing.security],
    ["setup", "trustedSetup", signing.setup],
    ["reproducible", "reproducibleBuild", signing.reproducible],
  ];
  const out = {};
  for (const [name, roleKey, signingOptions] of rows) {
    const body = {
      ...request.roles[roleKey].body,
      ...(overrides[name] ?? {}),
    };
    const filePath = join(outDir, `${name}.json`);
    await writeJson(filePath, signAttestationRecord(body, signingOptions));
    out[name] = filePath;
  }
  return out;
}

async function writeMaterialInputs(
  root,
  {
    r1csBytes = snarkjsMaterialBytes("r1cs", 3),
    includeToolchain = true,
  } = {},
) {
  const zkeyBytes = snarkjsMaterialBytes("zkey", 10, 96 * 1024);
  const ptauBytes = snarkjsMaterialBytes("ptau", 22, 96 * 1024);
  const snarkjsKey = verificationKey();
  const bscVerifierMaterial = snarkjsVerificationKeyToBscVerifierMaterial(
    snarkjsKey,
    { bscNetwork: "testnet" },
  );
  const r1cs = join(root, "full.r1cs");
  const zkey = join(root, "full.zkey");
  const ptau = join(root, "powersOfTau28_hez_final_22.ptau");
  const verificationKeyPath = join(root, "verification_key.json");
  const circuitSource = join(root, "full.circom");
  const witnessWasm = join(root, "full_js", "full.wasm");
  const trustedSetupTranscript = join(root, "trusted-setup-transcript.json");
  const reproducibleBuildTranscript = join(root, "reproducible-build-transcript.json");
  const circuitSourceText = generateBscFullMessageCircuitSource();
  const witnessWasmBytes = Buffer.from("sccp-bsc-test-witness-wasm");
  const reproducibleBuildToolchain = includeToolchain
    ? {
        circom: {
          source: "https://github.com/iden3/circom.git",
          tag: "v2.2.2",
          revision: "e410b0d5",
          binary: "circom",
          binarySha256: sha256Hex(Buffer.from("circom executable bytes")),
        },
        snarkjs: {
          package: "snarkjs",
          version: "0.7.6",
          binary: "snarkjs",
          binarySha256: sha256Hex(Buffer.from("snarkjs executable bytes")),
        },
        circomDependencies: {
          circomlib: "2.0.5",
          "@electron-labs/keccak-circom": "0.0.3",
        },
      }
    : null;
  const reproducibleBuildToolchainSha256 = reproducibleBuildToolchain
    ? sha256Hex(Buffer.from(canonicalJson(reproducibleBuildToolchain), "utf8"))
    : null;
  const trustedSetupTranscriptText = `${JSON.stringify(
    {
      schema: TRUSTED_SETUP_TRANSCRIPT_SCHEMA,
      contributors: ["ceremony-contributor-a", "ceremony-contributor-b"],
      localSingleContributor: false,
      toxicWasteDestroyed: true,
      ceremonyResult: "pass",
      phase1: {
        snarkjsPowersOfTauVerify: {
          completed: true,
        },
      },
      phase2: {
        finalZkeySha256: sha256Hex(zkeyBytes),
        snarkjsZkeyVerify: "ZKey Ok!",
      },
    },
    null,
    2,
  )}\n`;
  const reproducibleBuildTranscriptText = `${JSON.stringify(
    {
      schema: REPRODUCIBLE_BUILD_TRANSCRIPT_SCHEMA,
      independentRebuilders: ["independent-rebuilder-a", "independent-rebuilder-b"],
      reproducible: true,
      ...(reproducibleBuildToolchain
        ? { toolchain: reproducibleBuildToolchain }
        : {}),
      r1csInfoSource: "snarkjs-cli",
      r1csPublicInputCount: 9,
      r1csConstraintCount: 8192,
      zkeyVerify: true,
      zkeyVerifyResult: "ZKey Ok!",
    },
    null,
    2,
  )}\n`;
  await writeFile(r1cs, r1csBytes);
  await writeFile(zkey, zkeyBytes);
  await writeFile(ptau, ptauBytes);
  await writeJson(verificationKeyPath, snarkjsKey);
  await writeFile(circuitSource, circuitSourceText);
  await mkdir(dirname(witnessWasm), { recursive: true });
  await writeFile(witnessWasm, witnessWasmBytes);
  await writeFile(trustedSetupTranscript, trustedSetupTranscriptText);
  await writeFile(reproducibleBuildTranscript, reproducibleBuildTranscriptText);
  const snarkjsStub = await writeSnarkjsStub(root, verificationKeyPath);
  const bscVerifierKeySha256 = sha256Hex(verifierKeyBytesFor(bscVerifierMaterial));
  return {
    r1cs,
    zkey,
    ptau,
    snarkjsStub,
    verificationKeyPath,
    circuitSource,
    witnessWasm,
    trustedSetupTranscript,
    reproducibleBuildTranscript,
    context: {
      verifierKeyHash: bscVerifierMaterial.verifierKeyHash,
      circuitSourceSha256: sha256Hex(Buffer.from(circuitSourceText)),
      witnessWasmSha256: sha256Hex(witnessWasmBytes),
      r1csSha256: sha256Hex(r1csBytes),
      powersOfTauSha256: sha256Hex(ptauBytes),
      provingKeySha256: sha256Hex(zkeyBytes),
      snarkjsVerificationKeySha256: sha256Hex(
        Buffer.from(`${JSON.stringify(snarkjsKey, null, 2)}\n`),
      ),
      bscVerifierKeySha256,
      trustedSetupTranscriptSha256: sha256Hex(
        Buffer.from(trustedSetupTranscriptText),
      ),
      reproducibleBuildTranscriptSha256: sha256Hex(
        Buffer.from(reproducibleBuildTranscriptText),
      ),
      reproducibleBuildToolchainSha256,
      reproducibleBuildToolchain,
      selfChecks: withSnarkjsSelfCheckContext({
        verifierKeyHash: bscVerifierMaterial.verifierKeyHash,
      }).selfChecks,
    },
  };
}

async function writeSnarkjsStub(
  root,
  verificationKeyPath,
  {
    publicInputCount = 9,
    constraintCount = 8192,
    supportSetup = false,
    supportZkeyVerify = true,
    failZkeyVerify = false,
    failR1csInfo = false,
    supportProofSelfTest = false,
    failProofVerify = false,
    publicSignalsOverride = null,
    acceptInvalidWitnesses = false,
  } = {},
) {
  const stubPath = join(root, "snarkjs-stub.cjs");
  const proofSelfTestStatePath = join(root, "snarkjs-proof-self-test-state.json");
  const setupZkeyBytes = snarkjsMaterialBytes("zkey", 10, 96 * 1024);
  await writeFile(
    stubPath,
    `#!/usr/bin/env node
const { existsSync, readFileSync, writeFileSync } = require("node:fs");
const args = process.argv.slice(2);
const proofSelfTestStatePath = ${JSON.stringify(proofSelfTestStatePath)};
if (args[0] === "--help" || args[0] === "-h") {
  process.stdout.write("snarkjs stub help\\n");
  process.exit(0);
}
if (args[0] === "r1cs" && args[1] === "info" && (args[2] === "--help" || args[2] === "-h")) {
  process.stdout.write("snarkjs r1cs info stub help\\n");
  process.exit(0);
}
if (args[0] === "r1cs" && args[1] === "info" && args[2]) {
  if (${JSON.stringify(failR1csInfo)}) {
    process.stderr.write("forced r1cs info failure\\n");
    process.exit(2);
  }
  process.stdout.write("# of Constraints: ${constraintCount}\\n# of Public Inputs: ${publicInputCount}\\n");
  process.exit(0);
}
if (args[0] === "zkey" && args[1] === "export" && args[2] === "verificationkey" && args[3] && args[4]) {
  writeFileSync(args[4], readFileSync(${JSON.stringify(verificationKeyPath)}));
  process.exit(0);
}
if (${JSON.stringify(supportZkeyVerify)} && args[0] === "zkey" && args[1] === "verify" && args[2] && args[3] && args[4]) {
  if (${JSON.stringify(failZkeyVerify)}) {
    process.stderr.write("forced zkey verification failure\\n");
    process.exit(3);
  }
  process.stdout.write("ZKey Ok!\\n");
  process.exit(0);
}
if (${JSON.stringify(supportProofSelfTest)} && args[0] === "wtns" && args[1] === "calculate" && args[2] && args[3] && args[4]) {
  const input = JSON.parse(readFileSync(args[3], "utf8"));
  if (existsSync(proofSelfTestStatePath)) {
    const state = JSON.parse(readFileSync(proofSelfTestStatePath, "utf8"));
    if (!${JSON.stringify(acceptInvalidWitnesses)}) {
      if (JSON.stringify(input.publicSignals) !== JSON.stringify(state.publicSignals)) {
        process.stderr.write("forced adversarial public signal rejection\\n");
        process.exit(4);
      }
      for (const [key, value] of Object.entries(input)) {
        if (key.endsWith("Bits") && Array.isArray(value) && value.some((bit) => bit !== 0 && bit !== 1)) {
          process.stderr.write("forced adversarial non-boolean bit rejection\\n");
          process.exit(4);
        }
      }
    }
  } else {
    writeFileSync(proofSelfTestStatePath, JSON.stringify({ publicSignals: input.publicSignals }));
  }
  writeFileSync(args[4], JSON.stringify({ publicSignals: input.publicSignals, input }));
  process.exit(0);
}
if (${JSON.stringify(supportProofSelfTest)} && args[0] === "groth16" && args[1] === "prove" && args[2] && args[3] && args[4] && args[5]) {
  const witness = JSON.parse(readFileSync(args[3], "utf8"));
  const publicSignalsOverride = ${JSON.stringify(publicSignalsOverride)};
  const publicSignals = publicSignalsOverride || witness.publicSignals;
  const proof = {
    pi_a: ["1", "2", "1"],
    pi_b: [["3", "4"], ["5", "6"], ["1", "0"]],
    pi_c: ["7", "8", "1"],
    protocol: "groth16",
    curve: "bn128"
  };
  writeFileSync(args[4], JSON.stringify(proof));
  writeFileSync(args[5], JSON.stringify(publicSignals));
  writeFileSync(proofSelfTestStatePath, JSON.stringify({ publicSignals, proof }));
  process.exit(0);
}
if (${JSON.stringify(supportProofSelfTest)} && args[0] === "groth16" && args[1] === "verify" && args[2] && args[3] && args[4]) {
  if (${JSON.stringify(failProofVerify)}) {
    process.stderr.write("forced proof verification failure\\n");
    process.exit(3);
  }
  if (existsSync(proofSelfTestStatePath)) {
    const state = JSON.parse(readFileSync(proofSelfTestStatePath, "utf8"));
    const publicSignals = JSON.parse(readFileSync(args[3], "utf8"));
    const proof = JSON.parse(readFileSync(args[4], "utf8"));
    if (JSON.stringify(publicSignals) !== JSON.stringify(state.publicSignals)) {
      process.stderr.write("forced proof public signal verification failure\\n");
      process.exit(3);
    }
    if (JSON.stringify(proof) !== JSON.stringify(state.proof)) {
      process.stderr.write("forced proof object verification failure\\n");
      process.exit(3);
    }
  }
  process.stdout.write("OK!\\n");
  process.exit(0);
}
if (${JSON.stringify(supportSetup)} && args[0] === "groth16" && args[1] === "setup" && args[2] && args[3] && args[4]) {
  writeFileSync(args[4], Buffer.from(${JSON.stringify(setupZkeyBytes.toString("base64"))}, "base64"));
  process.exit(0);
}
if (${JSON.stringify(supportSetup)} && args[0] === "zkey" && args[1] === "contribute" && args[2] && args[3]) {
  writeFileSync(args[3], readFileSync(args[2]));
  process.exit(0);
}
process.stderr.write("unexpected snarkjs stub invocation: " + args.join(" ") + "\\n");
process.exit(2);
`,
    { mode: 0o755 },
  );
  await chmod(stubPath, 0o755);
  return stubPath;
}

async function writeCircomStub(
  root,
  {
    r1csBytes = snarkjsMaterialBytes("r1cs", 3),
    wasmBytes = Buffer.from("sccp-bsc-test-wasm"),
  } = {},
) {
  const stubPath = join(root, "circom-stub.cjs");
  await writeFile(
    stubPath,
    `#!/usr/bin/env node
const { mkdirSync, writeFileSync } = require("node:fs");
const { basename, join } = require("node:path");
const args = process.argv.slice(2);
if (args[0] === "--help" || args[0] === "-h") {
  process.stdout.write("circom stub help\\n");
  process.exit(0);
}
const source = args[0];
const outIndex = args.indexOf("-o");
if (!source || outIndex === -1 || !args[outIndex + 1]) {
  process.stderr.write("unexpected circom stub invocation: " + args.join(" ") + "\\n");
  process.exit(2);
}
const outDir = args[outIndex + 1];
const stem = basename(source, ".circom");
mkdirSync(join(outDir, stem + "_js"), { recursive: true });
writeFileSync(join(outDir, stem + ".r1cs"), Buffer.from(${JSON.stringify(r1csBytes.toString("base64"))}, "base64"));
writeFileSync(join(outDir, stem + ".sym"), "1,1,0,main.publicSignals[0]\\n");
writeFileSync(join(outDir, stem + "_js", stem + ".wasm"), Buffer.from(${JSON.stringify(wasmBytes.toString("base64"))}, "base64"));
process.exit(0);
`,
    { mode: 0o755 },
  );
  await chmod(stubPath, 0o755);
  return stubPath;
}

async function copyExecutableFixture(source, target) {
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, await readFile(source));
  await chmod(target, 0o755);
  return target;
}

async function writePreflightCandidate(root, options = {}) {
  const outDir = join(root, "out");
  const stem = BSC_FULL_SCCP_CIRCUIT_PROFILE;
  const bscNetwork = options.bscNetwork ?? options.manifest?.bscNetwork ?? "testnet";
  const chain = bscNetwork === "mainnet" ? "bsc-mainnet" : "bsc-testnet";
  const chainIdHex =
    bscNetwork === "mainnet" ? BSC_MAINNET_CHAIN_ID_HEX : BSC_TESTNET_CHAIN_ID_HEX;
  const networkIdHex =
    bscNetwork === "mainnet"
      ? BSC_MAINNET_NETWORK_ID_HEX
      : BSC_TESTNET_NETWORK_ID_HEX;
  await mkdir(join(outDir, `${stem}_js`), { recursive: true });
  const r1csBytes =
    options.r1csBytes ?? r1csHeaderMaterialBytes({ nConstraints: 2_154_897 });
  const zkeyBytes = options.zkeyBytes ?? snarkjsMaterialBytes("zkey", 10, 96 * 1024);
  const ptauBytes = options.ptauBytes ?? snarkjsMaterialBytes("ptau", 22, 96 * 1024);
  const snarkjsKey = verificationKey();
  const bscVerifierMaterial = {
    ...snarkjsVerificationKeyToBscVerifierMaterial(snarkjsKey, {
      bscNetwork,
    }),
    ...(options.verifierMaterial ?? {}),
  };
  const circuitSource = join(outDir, `${stem}.circom`);
  const r1cs = join(outDir, `${stem}.r1cs`);
  const ptau = join(outDir, "powersOfTau28_hez_final_22.ptau");
  const zkey = join(outDir, `${stem}.final.zkey`);
  const snarkjsVerificationKey = join(
    outDir,
    `${stem}.snarkjs-verification-key.json`,
  );
  const bscVerifierKey = join(outDir, `${bscNetwork}-bsc-groth16-verifier-key.json`);
  const manifest = join(outDir, `${bscNetwork}-bsc-groth16-material.manifest.json`);
  await writeFile(circuitSource, generateBscFullMessageCircuitSource());
  await writeFile(r1cs, r1csBytes);
  await writeFile(ptau, ptauBytes);
  await writeFile(zkey, zkeyBytes);
  await writeFile(join(outDir, `${stem}.sym`), "1,1,0,main.publicSignals[0]\n");
  await writeFile(
    join(outDir, `${stem}_js`, `${stem}.wasm`),
    Buffer.from("sccp-bsc-full-message-wasm"),
  );
  await writeJson(snarkjsVerificationKey, snarkjsKey);
  await writeJson(bscVerifierKey, bscVerifierMaterial);
  const circuitSourceSha256 = sha256Hex(await readFile(circuitSource));
  const r1csSha256 = sha256Hex(await readFile(r1cs));
  const ptauSha256 = sha256Hex(await readFile(ptau));
  const provingKeySha256 = sha256Hex(await readFile(zkey));
  const snarkjsVerificationKeySha256 = sha256Hex(
    await readFile(snarkjsVerificationKey),
  );
  const bscVerifierKeySha256 = sha256Hex(await readFile(bscVerifierKey));
  await writeJson(manifest, {
    schema: BSC_GROTH16_MATERIAL_MANIFEST_SCHEMA,
    routeId: "taira_bsc_xor",
    assetKey: "xor",
    bscNetwork,
    chain,
    chainIdHex,
    networkIdHex,
    circuitProfile: BSC_FULL_SCCP_CIRCUIT_PROFILE,
    proofBackend: "evm-groth16-bn254-v1",
    proofFamily: "stark-fri-v1",
    publicInputCount: 9,
    publicSignalNames: [...BSC_GROTH16_PUBLIC_SIGNAL_NAMES],
    verifierKeyHash: bscVerifierMaterial.verifierKeyHash,
    proofArtifactHash: r1csSha256,
    provingKeyHash: provingKeySha256,
    artifacts: {
      circuitSource: {
        path: circuitSource,
        sha256: circuitSourceSha256,
      },
      r1cs: {
        path: r1cs,
        sha256: r1csSha256,
      },
      powersOfTau: {
        path: ptau,
        sha256: ptauSha256,
      },
      provingKey: {
        path: zkey,
        sha256: provingKeySha256,
      },
      snarkjsVerificationKey: {
        path: snarkjsVerificationKey,
        sha256: snarkjsVerificationKeySha256,
      },
      bscVerifierKey: {
        path: bscVerifierKey,
        sha256: bscVerifierKeySha256,
      },
    },
    selfChecks: {
      circuitSource: {
        fullMessageCircuit: true,
        signalBindingFixture: false,
        unresolvedPlaceholders: false,
        keccakPublicSignalDerivation: true,
        digestReductionModuloScalarField: true,
        valueBitBooleanConstraints: true,
        publicSignalConstraintCount: 9,
        labelBindingCount: 9,
      },
      snarkjs: {
        r1csInfo: true,
        r1csInfoSource: "binary-header-fallback",
        r1csPublicInputCount: 9,
        r1csConstraintCount: 2_154_897,
        zkeyVerify: true,
        zkeyVerifyResult: "ZKey Ok!",
        zkeyVerificationKeyExport: true,
        verifierKeyHashMatches: true,
        exportedVerifierKeyHash: bscVerifierMaterial.verifierKeyHash,
      },
    },
    productionReady: true,
    productionBlockers: [],
    ...(options.manifest ?? {}),
  });
  return {
    outDir,
    circuitSource,
    r1cs,
    ptau,
    zkey,
    snarkjsVerificationKey,
    bscVerifierKey,
    manifest,
    verifierKeyHash: bscVerifierMaterial.verifierKeyHash,
    proofArtifactHash: r1csSha256,
    provingKeyHash: provingKeySha256,
  };
}

async function writeAttestationRequestCandidate(root, options = {}) {
  const inputs = await writeMaterialInputs(root, options.materialInputs ?? {});
  const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);
  const result = await materializeBscGroth16Material({
    "bsc-network": "testnet",
    r1cs: inputs.r1cs,
    zkey: inputs.zkey,
    "snarkjs-verifier-key": inputs.verificationKeyPath,
    "snarkjs-bin": snarkjsStub,
    "circuit-source": inputs.circuitSource,
    ...transcriptOptions(inputs),
    "out-dir": join(root, "out"),
  });
  const evidence =
    options.evidence === false
      ? null
      : await writeReviewEvidenceFiles(
          root,
          inputs.context,
          options.evidenceOverrides ?? {},
        );
  if (evidence) {
    result.attestationRequestEvidenceArgs = evidence.args;
    result.semanticReviewEvidence = evidence.semanticReviewEvidence;
    result.circuitSecurityAuditEvidence = evidence.circuitSecurityAuditEvidence;
  }
  result.reproducibleBuildToolchainSha256 =
    inputs.context.reproducibleBuildToolchainSha256;
  return { inputs, result, manifest: result.manifest, evidence };
}

test("BSC Groth16 material converter maps SnarkJS verifier key to Solidity constructor order", () => {
  const material = snarkjsVerificationKeyToBscVerifierMaterial(
    verificationKey(),
    { bscNetwork: "testnet" },
  );

  assert.equal(material.routeId, "taira_bsc_xor");
  assert.equal(material.bscNetwork, "testnet");
  assert.equal(material.networkIdHex, BSC_TESTNET_NETWORK_ID_HEX);
  assert.deepEqual(material.beta2, SOLIDITY_G2_GENERATOR);
  assert.deepEqual(material.gamma2, SOLIDITY_G2_GENERATOR);
  assert.deepEqual(material.delta2, SOLIDITY_G2_GENERATOR);
  assert.equal(material.ic.length, 20);
  assert.equal(material.publicInputCount, 9);
  assert.deepEqual(material.publicSignalNames, BSC_GROTH16_PUBLIC_SIGNAL_NAMES);
  assert.equal(material.verifierKeyHash, bscGroth16VerifierKeyHash(material));
  assert.equal(
    normalizeVerifierMaterial(material).expectedVerifierKeyHash,
    material.verifierKeyHash,
  );
});

test("BSC Groth16 material converter rejects verifier keys with wrong public input count", () => {
  assert.throws(
    () =>
      snarkjsVerificationKeyToBscVerifierMaterial(
        verificationKey({ nPublic: 8 }),
      ),
    /nPublic must be 9/u,
  );
  assert.throws(
    () =>
      snarkjsVerificationKeyToBscVerifierMaterial(
        verificationKey({ IC: Array.from({ length: 9 }, () => VALID_G1) }),
      ),
    /IC must contain exactly 10 G1 points/u,
  );
});

test("BSC Groth16 material converter rejects adversarial verifier keys", () => {
  assert.throws(
    () =>
      snarkjsVerificationKeyToBscVerifierMaterial(
        verificationKey({ protocol: "plonk" }),
      ),
    /protocol must be groth16/u,
  );
  assert.throws(
    () =>
      snarkjsVerificationKeyToBscVerifierMaterial(
        verificationKey({ curve: "bls12381" }),
      ),
    /curve must be bn128\/bn254/u,
  );
  assert.throws(
    () =>
      snarkjsVerificationKeyToBscVerifierMaterial(
        verificationKey({
          vk_beta_2: [
            ["1", "2"],
            ["3", "4"],
            ["1", "0"],
          ],
        }),
      ),
    /beta2 must be on the BN254 G2 twist curve/u,
  );
  assert.throws(
    () =>
      snarkjsVerificationKeyToBscVerifierMaterial(
        verificationKey({
          vk_alpha_1: [`0${VALID_G1[0]}`, VALID_G1[1], "1"],
        }),
      ),
    /vk_alpha_1\[0\] must be a canonical decimal BN254 field word/u,
  );
  assert.throws(
    () =>
      snarkjsVerificationKeyToBscVerifierMaterial(
        verificationKey({
          vk_alpha_1: [BN254_BASE_FIELD_MODULUS, VALID_G1[1], "1"],
        }),
      ),
    /vk_alpha_1\[0\] must be a BN254 field element/u,
  );
  assert.throws(
    () =>
      snarkjsVerificationKeyToBscVerifierMaterial(
        verificationKey({
          vk_gamma_2: [
            [SOLIDITY_G2_GENERATOR[0], SOLIDITY_G2_GENERATOR[1]],
            [SOLIDITY_G2_GENERATOR[2], SOLIDITY_G2_GENERATOR[3]],
            ["0", "1"],
          ],
        }),
      ),
    /vk_gamma_2\[2\] must be the projective one coordinate/u,
  );
});

test("BSC signal-binding circuit source keeps non-linear constraints and 9 public inputs", () => {
  const source = generateBscSignalBindingCircuitSource();

  assert.match(source, /signal input publicSignals\[9\]/u);
  assert.match(source, /signal input witnessSignals\[9\]/u);
  assert.match(source, /diff\[i\] \* diff\[i\] === 0/u);
  assert.match(source, /component main \{ public \[publicSignals\] \}/u);
});

test("BSC full message circuit source exposes Keccak-derived production signals", () => {
  const source = generateBscFullMessageCircuitSource();

  assert.match(source, /template SccpBscFullMessageV1/u);
  assert.match(source, /template SccpBscLabeledKeccakSignal/u);
  assert.match(source, /Keccak\(512, 256\)/u);
  assert.match(source, /circomlib\/circuits\/gates\.circom/u);
  assert.match(source, /circomlib\/circuits\/sha256\/xor3\.circom/u);
  assert.match(source, /circomlib\/circuits\/sha256\/shift\.circom/u);
  assert.match(source, /@electron-labs\/keccak-circom\/circuits\/keccak\.circom/u);
  assert.match(source, /publicSignal === digestBigEndianModFr/u);
  assert.match(source, /valueBits\[byte \* 8 \+ bit\] \* \(valueBits\[byte \* 8 \+ bit\] - 1\) === 0/u);
  for (let index = 0; index < 9; index += 1) {
    assert.match(source, new RegExp(`publicSignals\\[${index}\\]`, "u"));
  }
  assert.match(source, /message_id/u);
  assert.match(source, /payload_hash/u);
  assert.match(source, /target_domain/u);
  assert.match(source, /commitment_root/u);
  assert.match(source, /finality_height/u);
  assert.match(source, /finality_block_hash/u);
  assert.match(source, /source_domain/u);
  assert.match(source, /statement_hash/u);
  assert.match(source, /destination_binding_hash/u);
  assert.doesNotMatch(source, /SccpBscSignalBindingV1/u);
  assert.doesNotMatch(source, /witnessSignals/u);
  assert.doesNotMatch(source, /must connect|Implementations must wire|fixture fallback/u);
});

test("materialize writes verifier material but fails closed without production attestations", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-material-"));
  try {
    const r1cs = join(root, "candidate.r1cs");
    const zkey = join(root, "candidate.zkey");
    const verificationKeyPath = join(root, "verification_key.json");
    await writeFile(r1cs, Buffer.from("r1cs\x01\x00\x00\x00", "binary"));
    await writeFile(zkey, Buffer.from("zkey\x01\x00\x00\x00", "binary"));
    await writeFile(
      verificationKeyPath,
      `${JSON.stringify(verificationKey(), null, 2)}\n`,
    );

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      r1cs,
      zkey,
      "snarkjs-verifier-key": verificationKeyPath,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /missing semantic SCCP circuit attestation/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /missing trusted setup ceremony attestation/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /R1CS must be at least 65536 bytes/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /zkey must be at least 65536 bytes/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /SnarkJS zkey verify self-check requires a Powers of Tau artifact/u,
    );
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(manifest.circuitProfile, BSC_FULL_SCCP_CIRCUIT_PROFILE);
    assert.equal(manifest.productionReady, false);
    const verifier = JSON.parse(await readFile(result.verifierKey, "utf8"));
    assert.equal(verifier.verifierKeyHash, result.verifierKeyHash);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize rejects zkeys that fail Powers-of-Tau verification", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-zkey-verify-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath, {
      failZkeyVerify: true,
    });
    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      ptau: inputs.ptau,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      "trusted-setup-transcript": inputs.trustedSetupTranscript,
      "reproducible-build-transcript": inputs.reproducibleBuildTranscript,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /SnarkJS zkey verify self-check failed/u,
    );
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(manifest.selfChecks.snarkjs.zkeyVerify, false);
    assert.match(
      manifest.selfChecks.snarkjs.zkeyVerifyError,
      /forced zkey verification failure/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("generate refuses local setup unless explicitly constrained to testnet candidates", async () => {
  await assert.rejects(
    () =>
      main([
        "generate",
        "--bsc-network",
        "testnet",
        "--create-local-ptau-power",
        "8",
      ]),
    /requires --allow-local-testnet-setup true/u,
  );
  await assert.rejects(
    () =>
      main([
        "generate",
        "--bsc-network",
        "mainnet",
        "--create-local-ptau-power",
        "8",
        "--allow-local-testnet-setup",
        "true",
      ]),
    /only allowed for testnet candidates/u,
  );
});

test("generate uses checked-in canonical full-message source when source is omitted", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-full-generate-"));
  try {
    const ptau = join(root, "phase2.ptau");
    const verificationKeyPath = join(root, "verification_key.json");
    await writeFile(ptau, Buffer.from("ptau"));
    await writeJson(verificationKeyPath, verificationKey());
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(root, verificationKeyPath, {
      supportSetup: true,
    });
    const outDir = join(root, "out");

    const result = await generateBscGroth16Material({
      "bsc-network": "testnet",
      ptau,
      "circuit-profile": BSC_FULL_SCCP_CIRCUIT_PROFILE,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
      "out-dir": outDir,
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /missing semantic SCCP circuit attestation/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /phase2 zkey contribution is local single-contributor material/u,
    );
    const copiedSource = await readFile(
      join(outDir, `${BSC_FULL_SCCP_CIRCUIT_PROFILE}.circom`),
      "utf8",
    );
    const canonicalSource = await readFile(
      DEFAULT_BSC_FULL_MESSAGE_CIRCUIT_SOURCE,
      "utf8",
    );
    assert.equal(copiedSource, canonicalSource);
    assert.equal(canonicalSource, generateBscFullMessageCircuitSource());
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(manifest.circuitProfile, BSC_FULL_SCCP_CIRCUIT_PROFILE);
    assert.equal(manifest.selfChecks.circuitSource.fullMessageCircuit, true);
    assert.equal(manifest.selfChecks.circuitSource.labelBindingCount, 9);
    assert.match(
      manifest.artifacts.circuitSource.path,
      /sccp-bsc-full-message-v1\.circom$/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("toolchain-fingerprint writes exact executable hashes into transcript copies", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-toolchain-hash-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);
    const transcript = JSON.parse(
      await readFile(inputs.reproducibleBuildTranscript, "utf8"),
    );
    transcript.toolchain.circom.binary = circomStub;
    transcript.toolchain.circom.binary_sha256 = `0x${"11".repeat(32)}`;
    delete transcript.toolchain.circom.binarySha256;
    transcript.toolchain.snarkjs.binary = snarkjsStub;
    transcript.toolchain.snarkjs.binary_sha256 = `0x${"22".repeat(32)}`;
    delete transcript.toolchain.snarkjs.binarySha256;
    const transcriptPath = join(root, "reproducible-build-without-hashes.json");
    const outPath = join(root, "reproducible-build-with-hashes.json");
    await writeJson(transcriptPath, transcript);

    const result = await fingerprintBscGroth16Toolchain({
      transcript: transcriptPath,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
      out: outPath,
    });

    const updated = JSON.parse(await readFile(outPath, "utf8"));
    assert.equal(updated.schema, REPRODUCIBLE_BUILD_TRANSCRIPT_SCHEMA);
    assert.deepEqual(
      updated.independentRebuilders,
      transcript.independentRebuilders,
    );
    assert.equal(updated.toolchain.circom.binary, circomStub);
    assert.equal(updated.toolchain.snarkjs.binary, snarkjsStub);
    assert.equal(
      updated.toolchain.circom.binarySha256,
      sha256Hex(await readFile(circomStub)),
    );
    assert.equal(
      updated.toolchain.snarkjs.binarySha256,
      sha256Hex(await readFile(snarkjsStub)),
    );
    assert.equal(
      Object.prototype.hasOwnProperty.call(
        updated.toolchain.circom,
        "binary_sha256",
      ),
      false,
    );
    assert.equal(
      Object.prototype.hasOwnProperty.call(
        updated.toolchain.snarkjs,
        "binary_sha256",
      ),
      false,
    );
    assert.equal(
      result.toolchainSha256,
      sha256Hex(Buffer.from(canonicalJson(updated.toolchain), "utf8")),
    );

    const material = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      ptau: inputs.ptau,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      "trusted-setup-transcript": inputs.trustedSetupTranscript,
      "reproducible-build-transcript": outPath,
      "out-dir": join(root, "out"),
    });
    assert.doesNotMatch(
      material.productionBlockers.join("\n"),
      /binarySha256/u,
    );
    const manifest = JSON.parse(await readFile(material.manifest, "utf8"));
    assert.equal(manifest.selfChecks.snarkjs.snarkjsBinary, snarkjsStub);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("toolchain-fingerprint refuses unresolved executable paths", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-toolchain-missing-"));
  try {
    await assert.rejects(
      () =>
        fingerprintBscGroth16Toolchain({
          "circom-bin": join(root, "missing-circom"),
          "snarkjs-bin": join(root, "missing-snarkjs"),
          out: join(root, "toolchain.json"),
        }),
      /Circom compiler command .* could not be resolved to a readable executable/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("transcript-template writes artifact-bound drafts that remain materialization blockers", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-transcript-template-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const snarkjsBin = await copyExecutableFixture(
      inputs.snarkjsStub,
      join(root, "snarkjs-bin"),
    );
    const circomBin = await copyExecutableFixture(
      inputs.snarkjsStub,
      join(root, "circom-bin"),
    );
    const outDir = join(root, "transcripts");

    const result = await writeBscGroth16TranscriptTemplates({
      "bsc-network": "testnet",
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      ptau: inputs.ptau,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "circuit-source": inputs.circuitSource,
      "witness-wasm": inputs.witnessWasm,
      "circom-bin": circomBin,
      "snarkjs-bin": snarkjsBin,
      "out-dir": outDir,
    });
    const index = JSON.parse(await readFile(result.out, "utf8"));
    const setup = JSON.parse(
      await readFile(result.trustedSetupTranscript, "utf8"),
    );
    const reproducible = JSON.parse(
      await readFile(result.reproducibleBuildTranscript, "utf8"),
    );

    assert.equal(index.schema, BSC_GROTH16_TRANSCRIPT_TEMPLATE_PACKAGE_SCHEMA);
    assert.equal(index.draftsAreNotProductionReady, true);
    assert.equal(setup.schema, BSC_GROTH16_TRUSTED_SETUP_TRANSCRIPT_SCHEMA);
    assert.equal(setup.phase1.sha256, inputs.context.powersOfTauSha256);
    assert.equal(setup.phase2.finalZkeySha256, inputs.context.provingKeySha256);
    assert.equal(setup.snarkjsZkeyVerify, undefined);
    assert.equal(setup.ceremonyResult, "pending");
    assert.equal(reproducible.schema, BSC_GROTH16_REPRODUCIBLE_BUILD_TRANSCRIPT_SCHEMA);
    assert.equal(reproducible.toolchain.circom.binarySha256, sha256Hex(await readFile(circomBin)));
    assert.equal(reproducible.toolchain.snarkjs.binarySha256, sha256Hex(await readFile(snarkjsBin)));
    assert.equal(reproducible.r1cs.sha256, inputs.context.r1csSha256);
    assert.equal(reproducible.r1csInfoSource, "snarkjs-cli");
    assert.equal(reproducible.r1csPublicInputCount, 9);
    assert.equal(reproducible.r1csConstraintCount, 8192);
    assert.equal(reproducible.witnessWasm.sha256, inputs.context.witnessWasmSha256);
    assert.equal(reproducible.verificationKey.snarkjsSha256, inputs.context.snarkjsVerificationKeySha256);
    assert.equal(reproducible.verificationKey.verifierKeyHash, inputs.context.verifierKeyHash);
    assert.match(reproducible.commands[0], /materialize .*--ptau/u);
    assert.match(reproducible.commands[0], /--trusted-setup-transcript/u);
    assert.match(reproducible.commands[0], /--reproducible-build-transcript/u);
    assert.doesNotMatch(reproducible.commands[0], /--semantic-attestation/u);

    const materialized = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      ptau: inputs.ptau,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsBin,
      "circuit-source": inputs.circuitSource,
      "witness-wasm": inputs.witnessWasm,
      "trusted-setup-transcript": result.trustedSetupTranscript,
      "reproducible-build-transcript": result.reproducibleBuildTranscript,
      "out-dir": join(root, "out"),
    });
    const blockers = materialized.productionBlockers.join("\n");
    assert.equal(materialized.productionReady, false);
    assert.match(blockers, /trusted setup transcript contributors must record at least 2/u);
    assert.match(blockers, /trusted setup transcript toxicWasteDestroyed must be true/u);
    assert.match(blockers, /trusted setup transcript ceremonyResult must be pass/u);
    assert.match(blockers, /trusted setup transcript snarkjsPowersOfTauVerify\.completed must be true/u);
    assert.match(blockers, /trusted setup transcript snarkjsZkeyVerify must be ZKey Ok!/u);
    assert.match(blockers, /reproducible build transcript independentRebuilders must record at least 2/u);
    assert.match(blockers, /reproducible build transcript reproducible must be true/u);
    assert.match(blockers, /reproducible build transcript zkeyVerify must be true/u);
    assert.doesNotMatch(blockers, /sha256 must match|toolchain object is required|binarySha256/u);

    await assert.rejects(
      () =>
        writeBscGroth16TranscriptTemplates({
          "bsc-network": "testnet",
          r1cs: inputs.r1cs,
          zkey: inputs.zkey,
          ptau: inputs.ptau,
          "snarkjs-verifier-key": inputs.verificationKeyPath,
          "circuit-source": inputs.circuitSource,
          "witness-wasm": inputs.witnessWasm,
          "circom-bin": circomBin,
          "snarkjs-bin": snarkjsBin,
          "out-dir": outDir,
        }),
      /already exists; pass --overwrite true/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize records witness WASM artifacts for reproducible transcript binding", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-witness-bind-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const reproducibleTranscript = JSON.parse(
      await readFile(inputs.reproducibleBuildTranscript, "utf8"),
    );
    reproducibleTranscript.witnessWasm = {
      path: "full_js/full.wasm",
      sha256: inputs.context.witnessWasmSha256,
    };
    await writeJson(inputs.reproducibleBuildTranscript, reproducibleTranscript);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      ptau: inputs.ptau,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": inputs.snarkjsStub,
      "circuit-source": inputs.circuitSource,
      "witness-wasm": inputs.witnessWasm,
      "trusted-setup-transcript": inputs.trustedSetupTranscript,
      "reproducible-build-transcript": inputs.reproducibleBuildTranscript,
      "out-dir": join(root, "out"),
    });

    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(
      manifest.artifacts.witnessWasm.sha256,
      inputs.context.witnessWasmSha256,
    );
    assert.doesNotMatch(
      result.productionBlockers.join("\n"),
      /witnessWasm\.sha256 expected hash is unavailable/u,
    );
    const evidence = await writeReviewEvidenceFiles(root, inputs.context);
    const requestPath = join(root, "request-with-witness.json");
    const requestResult = await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...evidence.args,
      "--toolchain-sha256",
      inputs.context.reproducibleBuildToolchainSha256,
      "--out",
      requestPath,
    ]);
    const request = JSON.parse(await readFile(requestPath, "utf8"));
    assert.equal(requestResult.readyForSignature.reproducibleBuild, true);
    assert.equal(
      request.artifacts.witnessWasm.sha256,
      inputs.context.witnessWasmSha256,
    );
    const status = await auditBscGroth16AttestationStatus({
      request: requestPath,
      ...trustedSignerOption(),
    });
    assert.doesNotMatch(
      status.problems.join("\n"),
      /witnessWasm\.sha256 expected hash is unavailable/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses forged or unsafe production attestations", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-bad-attest-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context, {
      semantic: {
        r1csSha256: `0x${"12".repeat(32)}`,
      },
      setup: {
        localSingleContributor: true,
        minimumContributors: 1,
      },
    });

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /semantic SCCP circuit r1csSha256 must match/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup localSingleContributor must be false/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup minimumContributors must be at least 2/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses attestations repackaged across chain ids or circuit source", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-drift-attest-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context, {
      semantic: {
        chainIdHex: "0x38",
        proofBackend: "fixture-replayed-backend",
      },
      reproducible: {
        circuitSourceSha256: `0x${"34".repeat(32)}`,
        proofFamily: "fixture-proof-family",
      },
    });

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /semantic SCCP circuit chainIdHex must be 0x61/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /semantic SCCP circuit proofBackend must be evm-groth16-bn254-v1/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build circuitSourceSha256 must match/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build proofFamily must be stark-fri-v1/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses unsigned, untrusted, or tampered attestations", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-signed-attest-"));
  try {
    const runCase = async (name, configure, extraOptions = trustedSignerOption()) => {
      const caseRoot = join(root, name);
      await mkdir(caseRoot, { recursive: true });
      const inputs = await writeMaterialInputs(caseRoot);
      const attestations = await writeBoundAttestations(caseRoot, inputs.context);
      await configure?.({ root: caseRoot, inputs, attestations });
      const snarkjsStub = await writeSnarkjsStub(
        caseRoot,
        inputs.verificationKeyPath,
      );
      return materializeBscGroth16Material({
        "bsc-network": "testnet",
        ...extraOptions,
        r1cs: inputs.r1cs,
        zkey: inputs.zkey,
        "snarkjs-verifier-key": inputs.verificationKeyPath,
        "snarkjs-bin": snarkjsStub,
        "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
        "semantic-attestation": attestations.semantic,
        "circuit-security-attestation": attestations.security,
        "trusted-setup-attestation": attestations.setup,
        "reproducible-build-attestation": attestations.reproducible,
        "out-dir": join(caseRoot, "out"),
      });
    };

    const noTrustedSigner = await runCase(
      "no-trusted-signer",
      null,
      {},
    );
    assert.equal(noTrustedSigner.productionReady, false);
    assert.match(
      noTrustedSigner.productionBlockers.join("\n"),
      /semantic SCCP circuit attestation trusted attestation signer fingerprint is required/u,
    );

    const unsigned = await runCase("unsigned", async ({ attestations }) => {
      const semantic = JSON.parse(await readFile(attestations.semantic, "utf8"));
      delete semantic.signature;
      await writeJson(attestations.semantic, semantic);
    });
    assert.equal(unsigned.productionReady, false);
    assert.match(
      unsigned.productionBlockers.join("\n"),
      /semantic SCCP circuit attestation signature is required/u,
    );

    const untrusted = await runCase("untrusted", async ({ root, inputs, attestations }) => {
      await writeBoundAttestations(root, inputs.context, {
        signing: {
          privateKey: UNTRUSTED_ATTESTATION_SIGNER.privateKey,
          publicKeyPem: UNTRUSTED_ATTESTATION_PUBLIC_KEY_PEM,
          signerFingerprint: UNTRUSTED_ATTESTATION_SIGNER_FINGERPRINT,
        },
      });
      assert.equal(typeof attestations.semantic, "string");
    });
    assert.equal(untrusted.productionReady, false);
    assert.match(
      untrusted.productionBlockers.join("\n"),
      /semantic SCCP circuit attestation signature signerFingerprint is not trusted/u,
    );

    const tampered = await runCase("tampered", async ({ attestations }) => {
      const semantic = JSON.parse(await readFile(attestations.semantic, "utf8"));
      semantic.reviewedBy = "post-sign tamper";
      await writeJson(attestations.semantic, semantic);
    });
    assert.equal(tampered.productionReady, false);
    assert.match(
      tampered.productionBlockers.join("\n"),
      /semantic SCCP circuit attestation signature signedPayloadSha256 must match attestation body/u,
    );
    assert.match(
      tampered.productionBlockers.join("\n"),
      /semantic SCCP circuit attestation detached signature verification failed/u,
    );

    const badPayloadHash = await runCase("bad-payload-hash", async ({ root, inputs }) => {
      await writeBoundAttestations(root, inputs.context, {
        signing: {
          signedPayloadSha256: `0x${"42".repeat(32)}`,
        },
      });
    });
    assert.equal(badPayloadHash.productionReady, false);
    assert.match(
      badPayloadHash.productionBlockers.join("\n"),
      /signature signedPayloadSha256 must match attestation body/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses production attestations that reuse a signer across roles", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-reused-signer-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context, {
      signing: defaultAttestationSigning().semantic,
    });
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /production Groth16 attestation signers must be role-separated/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize rejects secret-like attestation contents before manifest write", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-secret-attest-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context);
    const semantic = JSON.parse(await readFile(attestations.semantic, "utf8"));
    semantic.privateKey = `0x${"11".repeat(32)}`;
    await writeJson(attestations.semantic, semantic);

    await assert.rejects(
      () =>
        materializeBscGroth16Material({
          "bsc-network": "testnet",
          ...trustedSignerOption(),
          r1cs: inputs.r1cs,
          zkey: inputs.zkey,
          "snarkjs-verifier-key": inputs.verificationKeyPath,
          "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
          "semantic-attestation": attestations.semantic,
          "circuit-security-attestation": attestations.security,
          "trusted-setup-attestation": attestations.setup,
          "reproducible-build-attestation": attestations.reproducible,
          "out-dir": join(root, "out"),
        }),
      /semantic SCCP circuit attestation\.privateKey must not contain/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses signal-binding circuits repackaged as full material", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-signal-repack-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const signalBindingSource = generateBscSignalBindingCircuitSource();
    await writeFile(inputs.circuitSource, signalBindingSource);
    const attestations = await writeBoundAttestations(root, {
      ...inputs.context,
      circuitSourceSha256: sha256Hex(Buffer.from(signalBindingSource)),
    });
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /must not use the signal-binding fixture circuit/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses placeholder full-message circuit sources", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-placeholder-source-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const placeholderSource = `pragma circom 2.1.6;
include "circomlib/circuits/bitify.circom";
include "@electron-labs/keccak-circom/circuits/keccak.circom";
template SccpBscFullMessageV1() {
  signal input publicSignals[9];
  component signalKeccak[9];
  for (var i = 0; i < 9; i++) {
    signalKeccak[i] = Keccak(512, 256);
  }
  // Implementations must wire label constants and publicSignals here.
}
component main { public [publicSignals] } = SccpBscFullMessageV1();
`;
    await writeFile(inputs.circuitSource, placeholderSource);
    const attestations = await writeBoundAttestations(root, {
      ...inputs.context,
      circuitSourceSha256: sha256Hex(Buffer.from(placeholderSource)),
    });
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /unresolved scaffold placeholders/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /constrain all 9 publicSignals entries/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /bind all 9 Solidity signal labels/u,
    );
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(manifest.selfChecks.circuitSource.unresolvedPlaceholders, true);
    assert.equal(manifest.selfChecks.circuitSource.publicSignalConstraintCount, 0);
    assert.equal(manifest.selfChecks.circuitSource.labelBindingCount, 0);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize marks full material ready only with bound attestations", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-good-attest-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context);
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, true);
    assert.deepEqual(result.productionBlockers, []);
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(manifest.productionReady, true);
    assert.equal(manifest.selfChecks.snarkjs.r1csInfo, true);
    assert.equal(manifest.selfChecks.snarkjs.r1csPublicInputCount, 9);
    assert.equal(manifest.selfChecks.snarkjs.r1csConstraintCount, 8192);
    assert.equal(manifest.selfChecks.snarkjs.zkeyVerificationKeyExport, true);
    assert.equal(manifest.selfChecks.snarkjs.verifierKeyHashMatches, true);
    assert.deepEqual(manifest.selfChecks.circuitSource, {
      fullMessageCircuit: true,
      signalBindingFixture: false,
      unresolvedPlaceholders: false,
      keccakPublicSignalDerivation: true,
      digestReductionModuloScalarField: true,
      valueBitBooleanConstraints: true,
      publicSignalConstraintCount: 9,
      labelBindingCount: 9,
    });
    assert.equal(
      manifest.selfChecks.snarkjs.exportedVerifierKeyHash,
      result.verifierKeyHash,
    );
    assert.deepEqual(manifest.attestationTrustPolicy, {
      signatureSchema: BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA,
      requiredAlgorithm: "ed25519",
      trustedSignerFingerprints: TRUSTED_ATTESTATION_SIGNER_FINGERPRINTS,
    });
    assert.equal(
      manifest.attestations.semanticSccpCircuit.schema,
      BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA,
    );
    const {
      signature: _semanticSignature,
      signatures: _semanticSignatures,
      ...semanticSignedBody
    } = JSON.parse(await readFile(attestations.semantic, "utf8"));
    assert.deepEqual(manifest.attestations.semanticSccpCircuit.signature, {
      verified: true,
      algorithm: "ed25519",
      signerFingerprint: TEST_ATTESTATION_SIGNER_FINGERPRINT,
      signedPayloadSha256: sha256Hex(
        Buffer.from(canonicalJson(semanticSignedBody)),
      ),
    });
    assert.equal(
      Object.prototype.hasOwnProperty.call(
        manifest.attestations.semanticSccpCircuit,
        "record",
      ),
      false,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses otherwise valid attestations without bound transcript artifacts", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-missing-transcripts-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context);
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup transcript artifact is required/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build transcript artifact is required/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses fixture-labelled production evidence with valid signatures", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-fixture-labels-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const setupTranscript = Buffer.from(
      `${JSON.stringify(
        {
          schema: "iroha-sccp-bsc-trusted-setup-transcript/test-fixture",
          contributors: ["ceremony-contributor-a", "ceremony-contributor-b"],
          localSingleContributor: false,
          toxicWasteDestroyed: true,
          ceremonyResult: "pass",
          phase1: {
            snarkjsPowersOfTauVerify: {
              completed: true,
            },
          },
          phase2: {
            snarkjsZkeyVerify: "ZKey Ok!",
          },
        },
        null,
        2,
      )}\n`,
      "utf8",
    );
    const reproducibleTranscript = Buffer.from(
      `${JSON.stringify(
        {
          schema: "iroha-sccp-bsc-reproducible-build-transcript/test-fixture",
          independentRebuilders: [
            "independent-rebuilder-a",
            "independent-rebuilder-b",
          ],
          reproducible: true,
          r1csInfoSource: "snarkjs-cli",
          r1csPublicInputCount: 9,
          r1csConstraintCount: 8192,
        },
        null,
        2,
      )}\n`,
      "utf8",
    );
    await writeFile(inputs.trustedSetupTranscript, setupTranscript);
    await writeFile(inputs.reproducibleBuildTranscript, reproducibleTranscript);
    inputs.context.trustedSetupTranscriptSha256 = sha256Hex(setupTranscript);
    inputs.context.reproducibleBuildTranscriptSha256 =
      sha256Hex(reproducibleTranscript);
    const attestations = await writeBoundAttestations(root, inputs.context, {
      security: {
        auditReportId: "fixture-security-audit",
      },
    });
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup transcript\.schema must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build transcript\.schema must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /circuit security attestation\.auditReportId must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses fixture-labelled manifest references with clean evidence bodies", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-ref-labels-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const fixtureR1cs = join(root, "fixture-full.r1cs");
    await writeFile(fixtureR1cs, await readFile(inputs.r1cs));
    const attestations = await writeBoundAttestations(root, inputs.context);
    const fixtureSemantic = join(root, "fixture-semantic-attestation.json");
    await writeFile(fixtureSemantic, await readFile(attestations.semantic));
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: fixtureR1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": fixtureSemantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /material manifest artifacts\.r1cs\.path must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /material manifest attestations\.semanticSccpCircuit\.path must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses signed attestation bodies with unknown shadow fields", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-shadow-fields-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context, {
      semantic: {
        semanticShadowDecision: true,
      },
      reproducible: {
        verifier_key_hash_alias: inputs.context.verifierKeyHash,
      },
    });
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /semantic SCCP circuit attestation contains unknown field: semanticShadowDecision/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build attestation contains unknown field: verifier_key_hash_alias/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses signed attestation bodies with duplicate aliases", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-alias-fields-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context, {
      semantic: {
        route_id: "taira_bsc_xor",
      },
      reproducible: {
        proof_artifact_hash: inputs.context.r1csSha256,
      },
    });
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    const blockers = result.productionBlockers.join("\n");
    assert.equal(result.productionReady, false);
    assert.match(
      blockers,
      /semantic SCCP circuit attestation routeId must not use multiple aliases: routeId, route_id/u,
    );
    assert.match(
      blockers,
      /reproducible build attestation r1csSha256 must not use multiple aliases: r1csSha256, proof_artifact_hash/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses attestations whose transcript hashes drift from artifacts", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-drift-transcripts-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context, {
      setup: {
        contributionTranscriptSha256: `0x${"ab".repeat(32)}`,
      },
      reproducible: {
        buildTranscriptSha256: `0x${"cd".repeat(32)}`,
      },
    });
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup contributionTranscriptSha256 must match/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build buildTranscriptSha256 must match/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses locally-scoped transcript contents even when hashes and signatures match", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-local-transcripts-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const badSetupTranscript = Buffer.from(
      `${JSON.stringify(
        {
          schema: "iroha-sccp-bsc-trusted-setup-transcript/test-fixture",
          contributors: ["local-candidate"],
          localSingleContributor: true,
          toxicWasteDestroyed: false,
          ceremonyResult: "candidate-only",
          phase1: {
            snarkjsPowersOfTauVerify: {
              completed: false,
            },
          },
        },
        null,
        2,
      )}\n`,
      "utf8",
    );
    const badReproducibleTranscript = Buffer.from(
      `${JSON.stringify(
        {
          schema: "iroha-sccp-bsc-reproducible-build-transcript/test-fixture",
          independentRebuilders: ["local-candidate"],
          reproducible: false,
          r1csInfoSource: "manual-inspection",
          r1csPublicInputCount: 8,
          r1csConstraintCount: 128,
        },
        null,
        2,
      )}\n`,
      "utf8",
    );
    await writeFile(inputs.trustedSetupTranscript, badSetupTranscript);
    await writeFile(inputs.reproducibleBuildTranscript, badReproducibleTranscript);
    inputs.context.trustedSetupTranscriptSha256 =
      sha256Hex(badSetupTranscript);
    inputs.context.reproducibleBuildTranscriptSha256 =
      sha256Hex(badReproducibleTranscript);
    const attestations = await writeBoundAttestations(root, inputs.context);
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup transcript contributors must record at least 2/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup transcript localSingleContributor must be false/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup transcript snarkjsPowersOfTauVerify\.completed must be true/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build transcript independentRebuilders must record at least 2/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build transcript r1csInfoSource must be snarkjs-cli/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses transcript claims without explicit ceremony and rebuild pass evidence", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-missing-transcript-pass-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const setupTranscript = Buffer.from(
      `${JSON.stringify(
        {
          schema: "iroha-sccp-bsc-trusted-setup-transcript/test-fixture",
          contributors: ["fixture-contributor-a", "fixture-contributor-b"],
          toxicWasteDestroyed: true,
        },
        null,
        2,
      )}\n`,
      "utf8",
    );
    const reproducibleTranscript = Buffer.from(
      `${JSON.stringify(
        {
          schema: "iroha-sccp-bsc-reproducible-build-transcript/test-fixture",
          independentRebuilders: ["fixture-rebuilder-a", "fixture-rebuilder-b"],
          r1csInfoSource: "snarkjs-cli",
          r1csPublicInputCount: 9,
          r1csConstraintCount: 8192,
        },
        null,
        2,
      )}\n`,
      "utf8",
    );
    await writeFile(inputs.trustedSetupTranscript, setupTranscript);
    await writeFile(inputs.reproducibleBuildTranscript, reproducibleTranscript);
    inputs.context.trustedSetupTranscriptSha256 = sha256Hex(setupTranscript);
    inputs.context.reproducibleBuildTranscriptSha256 =
      sha256Hex(reproducibleTranscript);
    const attestations = await writeBoundAttestations(root, inputs.context);
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup transcript localSingleContributor must be false/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup transcript ceremonyResult is required/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup transcript snarkjsPowersOfTauVerify block is required/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup transcript phase2 block is required/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build transcript reproducible must be true/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses stale transcript materialize commands without PTAU binding", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-stale-transcript-commands-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const staleMaterializeCommand =
      "node scripts/sccp_bsc_groth16_material.mjs materialize " +
      "--bsc-network testnet " +
      `--r1cs ${inputs.r1cs} ` +
      `--zkey ${inputs.zkey} ` +
      `--snarkjs-verifier-key ${inputs.verificationKeyPath} ` +
      `--circuit-source ${inputs.circuitSource} ` +
      `--out-dir ${join(root, "out")}`;
    const setupTranscript = JSON.parse(
      await readFile(inputs.trustedSetupTranscript, "utf8"),
    );
    setupTranscript.commands = [staleMaterializeCommand];
    const reproducibleTranscript = JSON.parse(
      await readFile(inputs.reproducibleBuildTranscript, "utf8"),
    );
    reproducibleTranscript.sourceBuildTranscript = {
      path: basename(inputs.circuitSource),
      sha256: `0x${"11".repeat(32)}`,
    };
    reproducibleTranscript.commands = [
      staleMaterializeCommand,
      "node scripts/sccp_bsc_taira_xor_deploy.mjs groth16-material materialize " +
        "--bsc-network testnet --r1cs full.r1cs --zkey full.zkey " +
        "--semantic-attestation semantic.json",
    ];
    const setupTranscriptText = Buffer.from(
      `${JSON.stringify(setupTranscript, null, 2)}\n`,
      "utf8",
    );
    const reproducibleTranscriptText = Buffer.from(
      `${JSON.stringify(reproducibleTranscript, null, 2)}\n`,
      "utf8",
    );
    await writeFile(inputs.trustedSetupTranscript, setupTranscriptText);
    await writeFile(inputs.reproducibleBuildTranscript, reproducibleTranscriptText);
    inputs.context.trustedSetupTranscriptSha256 =
      sha256Hex(setupTranscriptText);
    inputs.context.reproducibleBuildTranscriptSha256 =
      sha256Hex(reproducibleTranscriptText);
    const attestations = await writeBoundAttestations(root, inputs.context);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "ptau": inputs.ptau,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": inputs.snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    const blockers = result.productionBlockers.join("\n");
    assert.match(
      blockers,
      /trusted setup transcript commands\[0\] materialize command must include --ptau/u,
    );
    assert.match(
      blockers,
      /trusted setup transcript commands\[0\] materialize command must include --trusted-setup-transcript/u,
    );
    assert.match(
      blockers,
      /reproducible build transcript commands\[0\] materialize command must include --reproducible-build-transcript/u,
    );
    assert.match(
      blockers,
      /reproducible build transcript commands\[1\] materialize command must not pass signed attestation files directly/u,
    );
    assert.match(
      blockers,
      /reproducible build transcript sourceBuildTranscript\.sha256 must match/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses unsafe source build transcript references", async () => {
  const cases = [
    {
      name: "absolute",
      sourcePath: ({ inputs }) => inputs.circuitSource,
      pattern: /sourceBuildTranscript path must be a safe relative path/u,
    },
    {
      name: "url",
      sourcePath: () => "https://builds.example.invalid/source-build.json",
      pattern: /sourceBuildTranscript path must be a safe relative path/u,
    },
    {
      name: "encoded-parent",
      sourcePath: () => "%2e%2e/source-build.json",
      pattern: /sourceBuildTranscript path must be a safe relative path/u,
    },
    {
      name: "double-encoded-parent",
      sourcePath: () => "%252e%252e/source-build.json",
      pattern: /sourceBuildTranscript path must be a safe relative path/u,
    },
    {
      name: "encoded-separator",
      sourcePath: () => "source%2fbuild.json",
      pattern: /sourceBuildTranscript path must be a safe relative path/u,
    },
    {
      name: "backslash",
      sourcePath: () => "source\\build.json",
      pattern: /sourceBuildTranscript path must be a safe relative path/u,
    },
    {
      name: "current-segment",
      sourcePath: () => "./source-build.json",
      pattern: /sourceBuildTranscript path must be a safe relative path/u,
    },
    {
      name: "unknown-field",
      prepare: async ({ inputs }) => ({
        path: basename(inputs.circuitSource),
        sha256: sha256Hex(await readFile(inputs.circuitSource)),
        operatorDecision: "approve anyway",
      }),
      pattern: /sourceBuildTranscript contains unknown field: operatorDecision/u,
    },
    {
      name: "hash-alias",
      prepare: async ({ inputs }) => {
        const digest = sha256Hex(await readFile(inputs.circuitSource));
        return {
          path: basename(inputs.circuitSource),
          sha256: digest,
          hash: digest,
        };
      },
      pattern: /sourceBuildTranscript sha256 must not use multiple aliases: sha256, hash/u,
    },
    {
      name: "symlink",
      prepare: async ({ root, inputs }) => {
        await mkdir(join(root, "out"), { recursive: true });
        const linkPath = join(root, "out", "source-build-link.json");
        await symlink(inputs.circuitSource, linkPath);
        return {
          path: "source-build-link.json",
          sha256: sha256Hex(await readFile(inputs.circuitSource)),
        };
      },
      pattern: /sourceBuildTranscript must not be a symbolic link/u,
    },
    {
      name: "oversized",
      prepare: async ({ root }) => {
        await mkdir(join(root, "out"), { recursive: true });
        const largePath = join(root, "out", "source-build-large.json");
        await writeFile(largePath, Buffer.alloc(17 * 1024 * 1024, 0x62));
        return {
          path: "source-build-large.json",
          sha256: sha256Hex(await readFile(largePath)),
        };
      },
      pattern: /sourceBuildTranscript is .*maximum allowed/u,
    },
  ];

  for (const testCase of cases) {
    const root = await mkdtemp(
      join(tmpdir(), `iroha-bsc-groth16-source-build-${testCase.name}-`),
    );
    try {
      const inputs = await writeMaterialInputs(root);
      const reproducibleTranscript = JSON.parse(
        await readFile(inputs.reproducibleBuildTranscript, "utf8"),
      );
      const reference = testCase.prepare
        ? await testCase.prepare({ root, inputs })
        : {
            path: testCase.sourcePath({ root, inputs }),
            sha256: sha256Hex(await readFile(inputs.circuitSource)),
          };
      reproducibleTranscript.sourceBuildTranscript = reference;
      const reproducibleTranscriptText = Buffer.from(
        `${JSON.stringify(reproducibleTranscript, null, 2)}\n`,
        "utf8",
      );
      await writeFile(
        inputs.reproducibleBuildTranscript,
        reproducibleTranscriptText,
      );
      inputs.context.reproducibleBuildTranscriptSha256 =
        sha256Hex(reproducibleTranscriptText);
      const attestations = await writeBoundAttestations(root, inputs.context);

      const result = await materializeBscGroth16Material({
        "bsc-network": "testnet",
        ...trustedSignerOption(),
        r1cs: inputs.r1cs,
        zkey: inputs.zkey,
        ptau: inputs.ptau,
        "snarkjs-verifier-key": inputs.verificationKeyPath,
        "snarkjs-bin": inputs.snarkjsStub,
        "circuit-source": inputs.circuitSource,
        ...transcriptOptions(inputs),
        "semantic-attestation": attestations.semantic,
        "circuit-security-attestation": attestations.security,
        "trusted-setup-attestation": attestations.setup,
        "reproducible-build-attestation": attestations.reproducible,
        "out-dir": join(root, "out"),
      });

      assert.equal(result.productionReady, false, testCase.name);
      assert.match(
        result.productionBlockers.join("\n"),
        testCase.pattern,
        testCase.name,
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  }
});

test("materialize refuses transcript shadow fields and duplicate aliases", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-transcript-shape-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const setupTranscript = JSON.parse(
      await readFile(inputs.trustedSetupTranscript, "utf8"),
    );
    const reproducibleTranscript = JSON.parse(
      await readFile(inputs.reproducibleBuildTranscript, "utf8"),
    );
    setupTranscript.operatorOverride = true;
    setupTranscript.routeId = "taira_bsc_xor";
    setupTranscript.route_id = "taira_bsc_xor";
    setupTranscript.phase1.operatorDecision = "trust local ceremony";
    setupTranscript.phase1.snarkjs_powers_of_tau_verify =
      setupTranscript.phase1.snarkjsPowersOfTauVerify;
    setupTranscript.phase1.snarkjsPowersOfTauVerify.verifiedAt =
      "2026-01-01T00:00:00.000Z";
    setupTranscript.phase1.snarkjsPowersOfTauVerify.verified_at =
      "2026-01-01T00:00:00.000Z";
    setupTranscript.phase2.finalZkeySha256 = `0x${"31".repeat(32)}`;
    setupTranscript.phase2.final_zkey_sha256 = `0x${"31".repeat(32)}`;

    const sourceBuildReference = {
      path: basename(inputs.circuitSource),
      sha256: sha256Hex(await readFile(inputs.circuitSource)),
    };
    reproducibleTranscript.operatorOverride = true;
    reproducibleTranscript.routeId = "taira_bsc_xor";
    reproducibleTranscript.route_id = "taira_bsc_xor";
    reproducibleTranscript.sourceBuildTranscript = sourceBuildReference;
    reproducibleTranscript.source_build_transcript = sourceBuildReference;
    reproducibleTranscript.toolchain = {
      circom: {
        source: "https://github.com/iden3/circom.git",
        tag: "v2.2.2",
        revision: "e410b0d5",
        binary: "circom",
        operatorDecision: "accept unreviewed compiler",
      },
      snarkjs: {
        package: "snarkjs",
        version: "0.7.6",
        binary: "snarkjs",
        operatorDecision: "accept unreviewed prover",
      },
      circomDependencies: {
        circomlib: "2.0.5",
      },
      circom_dependencies: {
        circomlib: "2.0.5",
      },
    };
    reproducibleTranscript.circuit = {
      path: basename(inputs.circuitSource),
      sha256: sourceBuildReference.sha256,
      hash: sourceBuildReference.sha256,
      fullMessageCircuit: true,
      operatorDecision: "accept fixture circuit",
    };
    reproducibleTranscript.r1cs = {
      path: basename(inputs.r1cs),
      sha256: inputs.context.r1csSha256,
      hash: inputs.context.r1csSha256,
    };
    reproducibleTranscript.witnessWasm = {
      path: "witness.wasm",
      sha256: `0x${"32".repeat(32)}`,
      hash: `0x${"32".repeat(32)}`,
    };
    reproducibleTranscript.zkey = {
      finalSha256: inputs.context.provingKeySha256,
      final_sha256: inputs.context.provingKeySha256,
    };
    reproducibleTranscript.verificationKey = {
      verifierKeyHash: inputs.context.verifierKeyHash,
      verifier_key_hash: inputs.context.verifierKeyHash,
    };

    const setupTranscriptText = Buffer.from(
      `${JSON.stringify(setupTranscript, null, 2)}\n`,
      "utf8",
    );
    const reproducibleTranscriptText = Buffer.from(
      `${JSON.stringify(reproducibleTranscript, null, 2)}\n`,
      "utf8",
    );
    await writeFile(inputs.trustedSetupTranscript, setupTranscriptText);
    await writeFile(inputs.reproducibleBuildTranscript, reproducibleTranscriptText);
    inputs.context.trustedSetupTranscriptSha256 =
      sha256Hex(setupTranscriptText);
    inputs.context.reproducibleBuildTranscriptSha256 =
      sha256Hex(reproducibleTranscriptText);
    const attestations = await writeBoundAttestations(root, inputs.context);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      ptau: inputs.ptau,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": inputs.snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    const blockers = result.productionBlockers.join("\n");
    assert.match(
      blockers,
      /trusted setup transcript contains unknown field: operatorOverride/u,
    );
    assert.match(
      blockers,
      /trusted setup transcript routeId must not use multiple aliases: routeId, route_id/u,
    );
    assert.match(
      blockers,
      /trusted setup transcript phase1 contains unknown field: operatorDecision/u,
    );
    assert.match(
      blockers,
      /trusted setup transcript phase1 snarkjsPowersOfTauVerify must not use multiple aliases: snarkjsPowersOfTauVerify, snarkjs_powers_of_tau_verify/u,
    );
    assert.match(
      blockers,
      /trusted setup transcript snarkjsPowersOfTauVerify verifiedAt must not use multiple aliases: verifiedAt, verified_at/u,
    );
    assert.match(
      blockers,
      /trusted setup transcript phase2 finalZkeySha256 must not use multiple aliases: finalZkeySha256, final_zkey_sha256/u,
    );
    assert.match(
      blockers,
      /reproducible build transcript contains unknown field: operatorOverride/u,
    );
    assert.match(
      blockers,
      /reproducible build transcript routeId must not use multiple aliases: routeId, route_id/u,
    );
    assert.match(
      blockers,
      /reproducible build transcript sourceBuildTranscript must not use multiple aliases: sourceBuildTranscript, source_build_transcript/u,
    );
    assert.match(
      blockers,
      /reproducible build transcript toolchain circomDependencies must not use multiple aliases: circomDependencies, circom_dependencies/u,
    );
    assert.match(
      blockers,
      /reproducible build transcript toolchain\.circom contains unknown field: operatorDecision/u,
    );
    assert.match(
      blockers,
      /reproducible build transcript circuit sha256 must not use multiple aliases: sha256, hash/u,
    );
    assert.match(
      blockers,
      /reproducible build transcript verificationKey verifierKeyHash must not use multiple aliases: verifierKeyHash, verifier_key_hash/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses reproducible transcript artifact summary drift", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-transcript-artifacts-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const reproducibleTranscript = JSON.parse(
      await readFile(inputs.reproducibleBuildTranscript, "utf8"),
    );
    reproducibleTranscript.circuit = {
      path: basename(inputs.circuitSource),
      sha256: `0x${"41".repeat(32)}`,
      fullMessageCircuit: true,
      publicInputCount: 9,
    };
    reproducibleTranscript.r1cs = {
      path: basename(inputs.r1cs),
      sha256: `0x${"42".repeat(32)}`,
      nConstraints: 8192,
      nPublicInputs: 9,
    };
    reproducibleTranscript.zkey = {
      finalPath: basename(inputs.zkey),
      finalSha256: `0x${"43".repeat(32)}`,
    };
    reproducibleTranscript.verificationKey = {
      snarkjsPath: basename(inputs.verificationKeyPath),
      snarkjsSha256: `0x${"44".repeat(32)}`,
      verifierKeyHash: `0x${"45".repeat(32)}`,
    };
    const reproducibleTranscriptText = Buffer.from(
      `${JSON.stringify(reproducibleTranscript, null, 2)}\n`,
      "utf8",
    );
    await writeFile(inputs.reproducibleBuildTranscript, reproducibleTranscriptText);
    inputs.context.reproducibleBuildTranscriptSha256 =
      sha256Hex(reproducibleTranscriptText);
    const attestations = await writeBoundAttestations(root, inputs.context);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      ptau: inputs.ptau,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": inputs.snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    const blockers = result.productionBlockers.join("\n");
    assert.match(
      blockers,
      /reproducible build transcript circuit\.sha256 must match/u,
    );
    assert.match(
      blockers,
      /reproducible build transcript r1cs\.sha256 must match/u,
    );
    assert.match(
      blockers,
      /reproducible build transcript zkey\.finalSha256 must match/u,
    );
    assert.match(
      blockers,
      /reproducible build transcript verificationKey\.snarkjsSha256 must match/u,
    );
    assert.match(
      blockers,
      /reproducible build transcript verificationKey\.verifierKeyHash must match/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses duplicate JSON keys in transcript evidence", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-duplicate-transcript-keys-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const duplicateSetupTranscript = Buffer.from(
      `{
  "schema": "iroha-sccp-bsc-trusted-setup-transcript/test-fixture",
  "contributors": ["fixture-contributor-a", "fixture-contributor-b"],
  "localSingleContributor": false,
  "localSingleContributor": true,
  "toxicWasteDestroyed": true,
  "ceremonyResult": "pass",
  "phase1": {
    "snarkjsPowersOfTauVerify": {
      "completed": true
    }
  },
  "phase2": {
    "snarkjsZkeyVerify": "ZKey Ok!"
  }
}
`,
      "utf8",
    );
    const duplicateReproducibleTranscript = Buffer.from(
      `{
  "schema": "iroha-sccp-bsc-reproducible-build-transcript/test-fixture",
  "independentRebuilders": ["fixture-rebuilder-a", "fixture-rebuilder-b"],
  "reproducible": true,
  "reproducible": false,
  "r1csInfoSource": "snarkjs-cli",
  "r1csPublicInputCount": 9,
  "r1csConstraintCount": 8192
}
`,
      "utf8",
    );
    await writeFile(inputs.trustedSetupTranscript, duplicateSetupTranscript);
    await writeFile(
      inputs.reproducibleBuildTranscript,
      duplicateReproducibleTranscript,
    );
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup transcript could not be read as JSON: trusted setup transcript contains a duplicate JSON object key/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build transcript could not be read as JSON: reproducible build transcript contains a duplicate JSON object key/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses duplicate transcript contributors and rebuilders", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-duplicate-transcripts-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const setupTranscript = Buffer.from(
      `${JSON.stringify(
        {
          schema: "iroha-sccp-bsc-trusted-setup-transcript/test-fixture",
          contributors: ["same-ceremony-key", "same-ceremony-key"],
          localSingleContributor: false,
          toxicWasteDestroyed: true,
          ceremonyResult: "pass",
          phase1: {
            snarkjsPowersOfTauVerify: {
              completed: true,
            },
          },
          phase2: {
            snarkjsZkeyVerify: "ZKey Ok!",
          },
        },
        null,
        2,
      )}\n`,
      "utf8",
    );
    const reproducibleTranscript = Buffer.from(
      `${JSON.stringify(
        {
          schema: "iroha-sccp-bsc-reproducible-build-transcript/test-fixture",
          independentRebuilders: ["same-builder", "same-builder"],
          reproducible: true,
          r1csInfoSource: "snarkjs-cli",
          r1csPublicInputCount: 9,
          r1csConstraintCount: 8192,
        },
        null,
        2,
      )}\n`,
      "utf8",
    );
    await writeFile(inputs.trustedSetupTranscript, setupTranscript);
    await writeFile(inputs.reproducibleBuildTranscript, reproducibleTranscript);
    inputs.context.trustedSetupTranscriptSha256 = sha256Hex(setupTranscript);
    inputs.context.reproducibleBuildTranscriptSha256 =
      sha256Hex(reproducibleTranscript);
    const attestations = await writeBoundAttestations(root, inputs.context);
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /trusted setup transcript contributors must record at least 2/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build transcript independentRebuilders must record at least 2/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses reproducible build attestations with forged self-check metadata", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-forged-build-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context, {
      reproducible: {
        toolchainSha256: `0x${"00".repeat(32)}`,
        r1csInfoSource: "unreviewed-local-script",
        r1csPublicInputCount: 8,
        r1csConstraintCount: 128,
        zkeyVerificationKeyExport: false,
        verifierKeyHashMatches: false,
        exportedVerifierKeyHash: `0x${"44".repeat(32)}`,
      },
    });
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build r1csInfoSource must be snarkjs-cli/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build r1csPublicInputCount must be 9/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build r1csConstraintCount must be 8192/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build zkeyVerificationKeyExport must be true/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build verifierKeyHashMatches must be true/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /reproducible build exportedVerifierKeyHash must match/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize uses checked-in canonical full-message source when source is omitted", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-default-source-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context);
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath);

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, true);
    const canonicalSource = await readFile(
      DEFAULT_BSC_FULL_MESSAGE_CIRCUIT_SOURCE,
      "utf8",
    );
    assert.equal(canonicalSource, generateBscFullMessageCircuitSource());
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.match(
      manifest.artifacts.circuitSource.path,
      /sccp-bsc-full-message-v1\.circom$/u,
    );
    assert.equal(
      manifest.artifacts.circuitSource.sha256,
      inputs.context.circuitSourceSha256,
    );
    assert.equal(manifest.selfChecks.circuitSource.fullMessageCircuit, true);
    assert.equal(manifest.selfChecks.circuitSource.signalBindingFixture, false);
    assert.equal(manifest.selfChecks.circuitSource.labelBindingCount, 9);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize falls back to bounded R1CS header parsing when snarkjs info fails", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-r1cs-header-fallback-"));
  try {
    const r1csBytes = r1csHeaderMaterialBytes();
    const inputs = await writeMaterialInputs(root, { r1csBytes });
    const fallbackReproducibleTranscript = Buffer.from(
      `${JSON.stringify(
        {
          schema: REPRODUCIBLE_BUILD_TRANSCRIPT_SCHEMA,
          independentRebuilders: [
            "independent-rebuilder-a",
            "independent-rebuilder-b",
          ],
          reproducible: true,
          toolchain: inputs.context.reproducibleBuildToolchain,
          r1csInfoSource: "binary-header-fallback",
          r1csPublicInputCount: 9,
          r1csConstraintCount: 2_154_888,
          zkeyVerify: true,
          zkeyVerifyResult: "ZKey Ok!",
        },
        null,
        2,
      )}\n`,
      "utf8",
    );
    await writeFile(
      inputs.reproducibleBuildTranscript,
      fallbackReproducibleTranscript,
    );
    inputs.context.reproducibleBuildTranscriptSha256 = sha256Hex(
      fallbackReproducibleTranscript,
    );
    const attestations = await writeBoundAttestations(
      root,
      withSnarkjsSelfCheckContext(inputs.context, {
        r1csInfoSource: "binary-header-fallback",
        r1csConstraintCount: 2_154_888,
      }),
    );
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath, {
      failR1csInfo: true,
    });

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, true);
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(manifest.selfChecks.snarkjs.r1csInfo, true);
    assert.equal(
      manifest.selfChecks.snarkjs.r1csInfoSource,
      "binary-header-fallback",
    );
    assert.match(
      manifest.selfChecks.snarkjs.r1csInfoError,
      /forced r1cs info failure/u,
    );
    assert.equal(manifest.selfChecks.snarkjs.r1csPublicInputCount, 9);
    assert.equal(manifest.selfChecks.snarkjs.r1csConstraintCount, 2_154_888);
    assert.equal(manifest.selfChecks.snarkjs.r1csBinaryHeader.nPubInputs, 9);
    assert.equal(
      manifest.selfChecks.snarkjs.r1csBinaryHeader.nConstraints,
      2_154_888,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize fallback rejects forged R1CS headers with wrong signal counts", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-r1cs-header-bad-"));
  try {
    const r1csBytes = r1csHeaderMaterialBytes({
      nPubInputs: 8,
      nConstraints: 128,
    });
    const inputs = await writeMaterialInputs(root, { r1csBytes });
    const attestations = await writeBoundAttestations(
      root,
      withSnarkjsSelfCheckContext(inputs.context, {
        r1csInfoSource: "binary-header-fallback",
        r1csPublicInputCount: 8,
        r1csConstraintCount: 128,
      }),
    );
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath, {
      failR1csInfo: true,
    });

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /public input count must be 9/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /constraint count must be at least 4096/u,
    );
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(
      manifest.selfChecks.snarkjs.r1csInfoSource,
      "binary-header-fallback",
    );
    assert.equal(manifest.selfChecks.snarkjs.r1csPublicInputCount, 8);
    assert.equal(manifest.selfChecks.snarkjs.r1csConstraintCount, 128);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize fallback rejects R1CS headers without required sections", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-r1cs-header-sections-"));
  try {
    const r1csBytes = r1csHeaderMaterialBytes({
      includeConstraints: false,
    });
    const inputs = await writeMaterialInputs(root, { r1csBytes });
    const attestations = await writeBoundAttestations(root, inputs.context);
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath, {
      failR1csInfo: true,
    });

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /R1CS SnarkJS section 2 is required/u,
    );
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(manifest.selfChecks.snarkjs.r1csInfo, false);
    assert.match(
      manifest.selfChecks.snarkjs.r1csInfoError,
      /R1CS SnarkJS section 2 is required/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses full circuit material with wrong R1CS signal counts", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-r1cs-counts-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context);
    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath, {
      publicInputCount: 8,
      constraintCount: 128,
    });

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /public input count must be 9/u,
    );
    assert.match(
      result.productionBlockers.join("\n"),
      /constraint count must be at least 4096/u,
    );
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(manifest.selfChecks.snarkjs.r1csInfo, true);
    assert.equal(manifest.selfChecks.snarkjs.r1csPublicInputCount, 8);
    assert.equal(manifest.selfChecks.snarkjs.r1csConstraintCount, 128);
    assert.equal(manifest.selfChecks.snarkjs.verifierKeyHashMatches, true);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("materialize refuses zkeys that export a different verifier key", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-zkey-drift-"));
  try {
    const inputs = await writeMaterialInputs(root);
    const attestations = await writeBoundAttestations(root, inputs.context);
    const alternateVerificationKeyPath = join(root, "alternate_verification_key.json");
    await writeJson(
      alternateVerificationKeyPath,
      verificationKey({
        IC: Array.from({ length: 10 }, (_, index) =>
          index % 2 === 0 ? VALID_G1_ALT : VALID_G1,
        ),
      }),
    );
    const snarkjsStub = await writeSnarkjsStub(
      root,
      alternateVerificationKeyPath,
    );

    const result = await materializeBscGroth16Material({
      "bsc-network": "testnet",
      ...trustedSignerOption(),
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "snarkjs-bin": snarkjsStub,
      "circuit-source": inputs.circuitSource,
      ...transcriptOptions(inputs),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "out-dir": join(root, "out"),
    });

    assert.equal(result.productionReady, false);
    assert.match(
      result.productionBlockers.join("\n"),
      /zkey export hash mismatch/u,
    );
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    assert.equal(manifest.selfChecks.snarkjs.r1csInfo, true);
    assert.equal(manifest.selfChecks.snarkjs.zkeyVerificationKeyExport, true);
    assert.equal(manifest.selfChecks.snarkjs.verifierKeyHashMatches, false);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-request emits deterministic unsigned role payloads", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-request-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const toolchainSha256 = `0x${"ef".repeat(32)}`;
    const firstOut = join(root, "request-a.json");
    const secondOut = join(root, "request-b.json");

    const firstResult = await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      toolchainSha256,
      "--out",
      firstOut,
    ]);
    const secondResult = await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      toolchainSha256,
      "--out",
      secondOut,
    ]);

    assert.equal(firstResult.ok, true);
    assert.deepEqual(firstResult.readyForSignature, {
      semanticSccpCircuit: true,
      circuitSecurity: true,
      trustedSetup: true,
      reproducibleBuild: true,
    });
    assert.deepEqual(firstResult.readyForSignature, secondResult.readyForSignature);
    const firstPackage = JSON.parse(await readFile(firstOut, "utf8"));
    const secondPackage = JSON.parse(await readFile(secondOut, "utf8"));
    assert.deepEqual(firstPackage, secondPackage);
    assert.equal(
      firstPackage.schema,
      BSC_GROTH16_ATTESTATION_REQUEST_PACKAGE_SCHEMA,
    );
    assert.equal(firstPackage.manifest.sha256, sha256Hex(await readFile(result.manifest)));
    assert.equal(firstPackage.manifest.productionReady, false);
    assert.match(
      firstPackage.manifest.productionBlockers.join("\n"),
      /missing semantic SCCP circuit attestation/u,
    );
    assert.equal(firstPackage.routeId, "taira_bsc_xor");
    assert.equal(firstPackage.bscNetwork, "testnet");
    assert.deepEqual(
      firstPackage.publicSignalNames,
      BSC_GROTH16_PUBLIC_SIGNAL_NAMES,
    );
    assert.equal(
      firstPackage.roles.semanticSccpCircuit.body.schema,
      BSC_GROTH16_SEMANTIC_ATTESTATION_SCHEMA,
    );
    assert.equal(
      firstPackage.evidence.semanticReview.sha256,
      sha256Hex(await readFile(result.semanticReviewEvidence)),
    );
    assert.equal(
      firstPackage.evidence.circuitSecurityAudit.sha256,
      sha256Hex(await readFile(result.circuitSecurityAuditEvidence)),
    );
    assert.equal(
      firstPackage.roles.semanticSccpCircuit.body.semanticReviewEvidenceSchema,
      BSC_GROTH16_SEMANTIC_REVIEW_EVIDENCE_SCHEMA,
    );
    assert.equal(
      firstPackage.roles.semanticSccpCircuit.body.semanticReviewEvidenceSha256,
      firstPackage.evidence.semanticReview.sha256,
    );
    assert.equal(
      firstPackage.roles.semanticSccpCircuit.body.semanticReviewReportSha256,
      firstPackage.evidence.semanticReview.report.sha256,
    );
    assert.equal(
      firstPackage.roles.circuitSecurity.body.circuitSecurityAuditEvidenceSchema,
      BSC_GROTH16_CIRCUIT_SECURITY_AUDIT_EVIDENCE_SCHEMA,
    );
    assert.equal(
      firstPackage.roles.circuitSecurity.body.circuitSecurityAuditEvidenceSha256,
      firstPackage.evidence.circuitSecurityAudit.sha256,
    );
    assert.equal(
      firstPackage.roles.circuitSecurity.body.circuitSecurityAuditReportSha256,
      firstPackage.evidence.circuitSecurityAudit.report.sha256,
    );
    for (const role of Object.values(firstPackage.roles)) {
      assert.equal(role.body.proofBackend, "evm-groth16-bn254-v1");
      assert.equal(role.body.proofFamily, "stark-fri-v1");
    }
    assert.equal(
      firstPackage.roles.trustedSetup.body.contributionTranscriptSha256,
      firstPackage.artifacts.trustedSetupTranscript.sha256,
    );
    assert.equal(
      firstPackage.roles.reproducibleBuild.body.buildTranscriptSha256,
      firstPackage.artifacts.reproducibleBuildTranscript.sha256,
    );
    assert.equal(
      firstPackage.roles.reproducibleBuild.body.toolchainSha256,
      toolchainSha256,
    );
    for (const [roleName, role] of Object.entries(firstPackage.roles)) {
      assert.equal(
        role.signedPayloadSha256,
        sha256Hex(Buffer.from(canonicalJson(role.body), "utf8")),
        `${roleName} signedPayloadSha256`,
      );
      assert.equal(
        role.signatureTemplate.signedPayloadSha256,
        role.signedPayloadSha256,
        `${roleName} signature template hash`,
      );
    }
    assert.doesNotMatch(JSON.stringify(firstPackage), /privateKey|private_key/u);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("evidence-template writes manifest-bound drafts that remain unsigned blockers", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-evidence-template-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root, {
      evidence: false,
    });
    const outDir = join(root, "review-evidence");
    const requestPath = join(root, "request-from-drafts.json");

    const templateResult = await writeBscGroth16EvidenceTemplates({
      manifest: result.manifest,
      "out-dir": outDir,
    });
    const index = JSON.parse(await readFile(templateResult.out, "utf8"));
    const semanticEvidence = JSON.parse(
      await readFile(templateResult.semanticReviewEvidence, "utf8"),
    );
    const securityEvidence = JSON.parse(
      await readFile(templateResult.circuitSecurityAuditEvidence, "utf8"),
    );
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));

    assert.equal(index.schema, BSC_GROTH16_EVIDENCE_TEMPLATE_PACKAGE_SCHEMA);
    assert.equal(index.manifest.sha256, sha256Hex(await readFile(result.manifest)));
    assert.equal(index.draftsAreNotSignable, true);
    assert.equal(
      index.outputs.semanticReviewEvidence.sha256,
      sha256Hex(await readFile(templateResult.semanticReviewEvidence)),
    );
    assert.equal(
      index.outputs.circuitSecurityAuditEvidence.sha256,
      sha256Hex(await readFile(templateResult.circuitSecurityAuditEvidence)),
    );
    assert.equal(
      semanticEvidence.schema,
      BSC_GROTH16_SEMANTIC_REVIEW_EVIDENCE_SCHEMA,
    );
    assert.equal(semanticEvidence.reviewResult, "pending");
    assert.equal(semanticEvidence.fullSccpMessageSemantics, false);
    assert.equal(manifest.proofArtifactHash, manifest.artifacts.r1cs.sha256);
    assert.equal(
      manifest.provingKeyHash,
      manifest.artifacts.provingKey.sha256,
    );
    assert.equal(result.proofArtifactHash, manifest.proofArtifactHash);
    assert.equal(result.provingKeyHash, manifest.provingKeyHash);
    assert.equal(semanticEvidence.r1csSha256, manifest.artifacts.r1cs.sha256);
    assert.equal(
      semanticEvidence.provingKeySha256,
      manifest.artifacts.provingKey.sha256,
    );
    assert.equal(semanticEvidence.verifierKeyHash, result.verifierKeyHash);
    assert.equal(
      semanticEvidence.reviewReport.sha256,
      sha256Hex(await readFile(templateResult.semanticReviewReport)),
    );
    assert.equal(
      securityEvidence.schema,
      BSC_GROTH16_CIRCUIT_SECURITY_AUDIT_EVIDENCE_SCHEMA,
    );
    assert.equal(securityEvidence.auditResult, "pending");
    assert.equal(securityEvidence.approved, false);
    assert.equal(
      securityEvidence.auditReport.sha256,
      sha256Hex(await readFile(templateResult.circuitSecurityAuditReport)),
    );

    const requestResult = await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      "--semantic-review-evidence",
      templateResult.semanticReviewEvidence,
      "--circuit-security-audit-evidence",
      templateResult.circuitSecurityAuditEvidence,
      "--toolchain-sha256",
      `0x${"ef".repeat(32)}`,
      "--out",
      requestPath,
    ]);
    const request = JSON.parse(await readFile(requestPath, "utf8"));

    assert.equal(requestResult.readyForSignature.semanticSccpCircuit, false);
    assert.equal(requestResult.readyForSignature.circuitSecurity, false);
    assert.match(
      request.roles.semanticSccpCircuit.blockers.join("\n"),
      /reviewResult must be pass/u,
    );
    assert.match(
      request.roles.semanticSccpCircuit.blockers.join("\n"),
      /fullSccpMessageSemantics must be true/u,
    );
    assert.match(
      request.roles.circuitSecurity.blockers.join("\n"),
      /auditResult must be pass/u,
    );
    assert.match(
      request.roles.circuitSecurity.blockers.join("\n"),
      /approved must be true/u,
    );
    const status = await auditBscGroth16AttestationStatus({
      request: requestPath,
      ...trustedSignerOption(),
    });
    assert.equal(status.requestValid, true);
    assert.equal(status.requestReadyForSignature.semanticSccpCircuit, false);
    assert.equal(status.requestReadyForSignature.circuitSecurity, false);
    assert.match(
      status.roles.semanticSccpCircuit.blockers.join("\n"),
      /reviewResult must be pass/u,
    );
    assert.match(
      status.roles.circuitSecurity.blockers.join("\n"),
      /auditResult must be pass/u,
    );
    assert.doesNotMatch(
      status.problems.join("\n"),
      /semanticReviewEvidenceSchema is required|circuitSecurityAuditEvidenceSchema is required|body evidence sha256/u,
    );

    await assert.rejects(
      () =>
        writeBscGroth16EvidenceTemplates({
          manifest: result.manifest,
          "out-dir": outDir,
        }),
      /already exists; pass --overwrite true/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("handoff-bundle writes a hash-bound public readiness packet", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-handoff-"));
  try {
    const { inputs, result } = await writeAttestationRequestCandidate(root, {
      evidence: false,
    });
    const materialDir = dirname(result.manifest);
    const snarkjsBin = await copyExecutableFixture(
      inputs.snarkjsStub,
      join(root, "snarkjs-bin"),
    );
    const circomBin = await copyExecutableFixture(
      inputs.snarkjsStub,
      join(root, "circom-bin"),
    );
    const transcriptPackage = await writeBscGroth16TranscriptTemplates({
      "bsc-network": "testnet",
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      ptau: inputs.ptau,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "circuit-source": inputs.circuitSource,
      "witness-wasm": inputs.witnessWasm,
      "circom-bin": circomBin,
      "snarkjs-bin": snarkjsBin,
      "out-dir": join(materialDir, "transcripts"),
    });
    const evidencePackage = await writeBscGroth16EvidenceTemplates({
      manifest: result.manifest,
    });
    const requestPath = join(materialDir, "testnet-bsc-groth16-attestation-request.json");
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      "--semantic-review-evidence",
      evidencePackage.semanticReviewEvidence,
      "--circuit-security-audit-evidence",
      evidencePackage.circuitSecurityAuditEvidence,
      "--out",
      requestPath,
    ]);
    const out = join(root, "handoff.json");

    const handoffResult = await writeBscGroth16AttestationHandoff({
      manifest: result.manifest,
      out,
    });
    const handoff = JSON.parse(await readFile(out, "utf8"));
    const verifiedHandoff = await verifyBscGroth16AttestationHandoff({
      handoff: out,
    });

    assert.equal(handoffResult.ok, true);
    assert.equal(handoffResult.handoffComplete, true);
    assert.equal(handoffResult.productionReady, false);
    assert.equal(handoffResult.signingReady, false);
    assert.equal(handoff.schema, BSC_GROTH16_ATTESTATION_HANDOFF_SCHEMA);
    assert.equal(handoff.manifest.sha256, sha256Hex(await readFile(result.manifest)));
    assert.equal(
      handoff.packages.transcriptTemplates.sha256,
      sha256Hex(await readFile(transcriptPackage.out)),
    );
    assert.equal(
      handoff.packages.evidenceTemplates.sha256,
      sha256Hex(await readFile(evidencePackage.out)),
    );
    assert.equal(
      handoff.packages.attestationRequest.sha256,
      sha256Hex(await readFile(requestPath)),
    );
    assert.equal(handoff.packages.transcriptTemplates.draftsAreNotProductionReady, true);
    assert.equal(handoff.packages.evidenceTemplates.draftsAreNotSignable, true);
    assert.equal(handoff.readiness.handoffComplete, true);
    assert.equal(handoff.readiness.productionReady, false);
    assert.equal(handoff.readiness.signingReady, false);
    assert.equal(handoff.readiness.requestValid, true);
    assert.equal(
      handoff.readiness.requestReadyForSignature.semanticSccpCircuit,
      false,
    );
    assert.equal(handoff.readiness.missingSignedRoles.length, 4);
    assert.equal(verifiedHandoff.ok, true);
    assert.equal(
      verifiedHandoff.valid,
      true,
      verifiedHandoff.referenceBlockers.join("\n"),
    );
    assert.equal(verifiedHandoff.readiness.handoffComplete, true);
    assert.equal(verifiedHandoff.readiness.productionReady, false);
    assert.equal(verifiedHandoff.readiness.signingReady, false);
    assert.equal(verifiedHandoff.problemCount, handoff.readiness.problemCount);
    assert.equal(verifiedHandoff.referenceBlockers.length, 0);
    assert.match(
      handoff.readiness.attestationStatusProblems.join("\n"),
      /semantic SCCP circuit role is not ready for signature/u,
    );
    assert.match(
      handoff.readiness.attestationStatusProblems.join("\n"),
      /circuit security role is not ready for signature/u,
    );
    assert.match(
      handoff.commands.finalizeAttestations,
      /finalize-attestations .*--trusted-attestation-signer <0x\.\.\.>/u,
    );
    assert.match(
      handoff.commands.verifyHandoff,
      /verify-handoff --handoff .*handoff\.json --trusted-attestation-signer <0x\.\.\.>/u,
    );
    assert.doesNotMatch(JSON.stringify(handoff), /privateKey|private_key|mnemonic/u);

    await assert.rejects(
      () =>
        writeBscGroth16AttestationHandoff({
          manifest: result.manifest,
          out,
      }),
      /already exists; pass --overwrite true/u,
    );

    const forgedHandoffPath = join(root, "forged-handoff.json");
    await writeJson(forgedHandoffPath, {
      ...handoff,
      readiness: {
        ...handoff.readiness,
        productionReady: true,
      },
    });
    const forgedVerifiedHandoff = await verifyBscGroth16AttestationHandoff({
      handoff: forgedHandoffPath,
    });
    assert.equal(forgedVerifiedHandoff.valid, false);
    assert.match(
      forgedVerifiedHandoff.referenceBlockers.join("\n"),
      /readiness\.productionReady must match material manifest/u,
    );

    const forgedPackageSummaryPath = join(root, "forged-package-summary-handoff.json");
    await writeJson(forgedPackageSummaryPath, {
      ...handoff,
      packages: {
        ...handoff.packages,
        transcriptTemplates: {
          ...handoff.packages.transcriptTemplates,
          draftsAreNotProductionReady: false,
        },
      },
    });
    const forgedPackageSummaryVerification =
      await verifyBscGroth16AttestationHandoff({
        handoff: forgedPackageSummaryPath,
      });
    assert.equal(forgedPackageSummaryVerification.valid, false);
    assert.match(
      forgedPackageSummaryVerification.referenceBlockers.join("\n"),
      /transcript template package handoff draftsAreNotProductionReady must match referenced package/u,
    );

    const forgedPackageSchemaPath = join(root, "forged-package-schema-handoff.json");
    const {
      schema: _omittedTranscriptSummarySchema,
      ...transcriptTemplateSummaryWithoutSchema
    } = handoff.packages.transcriptTemplates;
    await writeJson(forgedPackageSchemaPath, {
      ...handoff,
      packages: {
        ...handoff.packages,
        transcriptTemplates: transcriptTemplateSummaryWithoutSchema,
      },
    });
    const forgedPackageSchemaVerification =
      await verifyBscGroth16AttestationHandoff({
        handoff: forgedPackageSchemaPath,
      });
    assert.equal(forgedPackageSchemaVerification.valid, false);
    assert.match(
      forgedPackageSchemaVerification.referenceBlockers.join("\n"),
      /transcript template package handoff schema must be iroha-sccp-bsc-groth16-transcript-template-package\/v1/u,
    );

    const forgedManifestSummaryPath = join(root, "forged-manifest-summary-handoff.json");
    await writeJson(forgedManifestSummaryPath, {
      ...handoff,
      manifest: {
        ...handoff.manifest,
        productionReady: true,
        productionBlockers: [],
      },
    });
    const forgedManifestSummaryVerification =
      await verifyBscGroth16AttestationHandoff({
        handoff: forgedManifestSummaryPath,
      });
    assert.equal(forgedManifestSummaryVerification.valid, false);
    assert.match(
      forgedManifestSummaryVerification.referenceBlockers.join("\n"),
      /manifest\.productionReady must match material manifest/u,
    );
    assert.match(
      forgedManifestSummaryVerification.referenceBlockers.join("\n"),
      /manifest\.productionBlockers must match material manifest/u,
    );

    const forgedReadinessSummaryPath = join(root, "forged-readiness-summary-handoff.json");
    await writeJson(forgedReadinessSummaryPath, {
      ...handoff,
      readiness: {
        ...handoff.readiness,
        requestReadyForSignature: {},
        missingSignedRoles: [],
        problemCount: 0,
        attestationStatusProblems: [],
        productionBlockers: [],
      },
    });
    const forgedReadinessSummaryVerification =
      await verifyBscGroth16AttestationHandoff({
        handoff: forgedReadinessSummaryPath,
      });
    assert.equal(forgedReadinessSummaryVerification.valid, false);
    assert.match(
      forgedReadinessSummaryVerification.referenceBlockers.join("\n"),
      /readiness\.problemCount must match verified handoff status/u,
    );
    assert.match(
      forgedReadinessSummaryVerification.referenceBlockers.join("\n"),
      /readiness\.requestReadyForSignature must match attestation status/u,
    );
    assert.match(
      forgedReadinessSummaryVerification.referenceBlockers.join("\n"),
      /readiness\.missingSignedRoles must match attestation status/u,
    );
    assert.match(
      forgedReadinessSummaryVerification.referenceBlockers.join("\n"),
      /readiness\.attestationStatusProblems must match attestation status/u,
    );
    assert.match(
      forgedReadinessSummaryVerification.referenceBlockers.join("\n"),
      /readiness\.productionBlockers must match material manifest/u,
    );

    const forgedShapeHandoffPath = join(root, "forged-shape-handoff.json");
    await writeJson(forgedShapeHandoffPath, {
      ...handoff,
      production_ready: true,
      manifest: {
        ...handoff.manifest,
        production_ready: true,
      },
      packages: {
        ...handoff.packages,
        transcriptTemplates: {
          ...handoff.packages.transcriptTemplates,
          drafts_are_not_production_ready: false,
        },
      },
      readiness: {
        ...handoff.readiness,
        signing_ready: true,
      },
      commands: {
        ...handoff.commands,
        verify_handoff: "operator-forged command",
        shadowFinalize: "operator-forged command",
      },
    });
    const forgedShapeVerification = await verifyBscGroth16AttestationHandoff({
      handoff: forgedShapeHandoffPath,
    });
    assert.equal(forgedShapeVerification.valid, false);
    assert.match(
      forgedShapeVerification.referenceBlockers.join("\n"),
      /attestation handoff contains unknown field: production_ready/u,
    );
    assert.match(
      forgedShapeVerification.referenceBlockers.join("\n"),
      /manifest summary contains unknown field: production_ready/u,
    );
    assert.match(
      forgedShapeVerification.referenceBlockers.join("\n"),
      /transcript template package handoff summary contains unknown field: drafts_are_not_production_ready/u,
    );
    assert.match(
      forgedShapeVerification.referenceBlockers.join("\n"),
      /attestation handoff readiness contains unknown field: signing_ready/u,
    );
    assert.match(
      forgedShapeVerification.referenceBlockers.join("\n"),
      /attestation handoff commands contains unknown field: shadowFinalize/u,
    );
    assert.match(
      forgedShapeVerification.referenceBlockers.join("\n"),
      /attestation handoff commands verifyHandoff must not use multiple aliases: verifyHandoff, verify_handoff/u,
    );

    const forgedCommandSummaryPath = join(root, "forged-command-summary-handoff.json");
    await writeJson(forgedCommandSummaryPath, {
      ...handoff,
      commands: {
        ...handoff.commands,
        signAttestation:
          `node scripts/sccp_bsc_groth16_material.mjs sign-attestation --request ${handoff.packages.attestationRequest.path} --role semanticSccpCircuit --out <signed-role-attestation.json>`,
        finalizeAttestations:
          `node scripts/sccp_bsc_groth16_material.mjs finalize-attestations --request ${handoff.packages.attestationRequest.path} --out-dir ${materialDir}`,
      },
    });
    const forgedCommandSummaryVerification =
      await verifyBscGroth16AttestationHandoff({
        handoff: forgedCommandSummaryPath,
      });
    assert.equal(forgedCommandSummaryVerification.valid, false);
    assert.match(
      forgedCommandSummaryVerification.referenceBlockers.join("\n"),
      /commands\.signAttestation must include --role semanticSccpCircuit\|circuitSecurity\|trustedSetup\|reproducibleBuild/u,
    );
    assert.match(
      forgedCommandSummaryVerification.referenceBlockers.join("\n"),
      /commands\.signAttestation must include --private-key-pem <ed25519-private-key\.pem>/u,
    );
    assert.match(
      forgedCommandSummaryVerification.referenceBlockers.join("\n"),
      /commands\.finalizeAttestations must include --semantic-attestation <semantic-sccp-circuit-attestation\.json>/u,
    );
    assert.match(
      forgedCommandSummaryVerification.referenceBlockers.join("\n"),
      /commands\.finalizeAttestations must include --trusted-attestation-signer <0x\.\.\.>/u,
    );

    const absoluteHandoffPath = join(root, "absolute-handoff-path.json");
    await writeJson(absoluteHandoffPath, {
      ...handoff,
      packages: {
        ...handoff.packages,
        transcriptTemplates: {
          ...handoff.packages.transcriptTemplates,
          path: transcriptPackage.out,
        },
      },
    });
    const absoluteHandoffVerification =
      await verifyBscGroth16AttestationHandoff({
        handoff: absoluteHandoffPath,
      });
    assert.equal(absoluteHandoffVerification.valid, false);
    assert.match(
      absoluteHandoffVerification.referenceBlockers.join("\n"),
      /transcript template package handoff reference path must be a safe relative path/u,
    );

    const parentTraversalHandoffPath = join(root, "parent-traversal-handoff-path.json");
    await writeJson(parentTraversalHandoffPath, {
      ...handoff,
      manifest: {
        ...handoff.manifest,
        path: "../manifest.json",
      },
    });
    const parentTraversalHandoffVerification =
      await verifyBscGroth16AttestationHandoff({
        handoff: parentTraversalHandoffPath,
      });
    assert.equal(parentTraversalHandoffVerification.valid, false);
    assert.match(
      parentTraversalHandoffVerification.referenceBlockers.join("\n"),
      /material manifest handoff reference path must be a safe relative path/u,
    );

    const staleTranscriptPackage = JSON.parse(
      await readFile(transcriptPackage.out, "utf8"),
    );
    staleTranscriptPackage.generatedAt = "2026-01-01T00:00:00.000Z";
    await writeJson(transcriptPackage.out, staleTranscriptPackage);
    const staleVerifiedHandoff = await verifyBscGroth16AttestationHandoff({
      handoff: out,
    });
    assert.equal(staleVerifiedHandoff.valid, false);
    assert.match(
      staleVerifiedHandoff.referenceBlockers.join("\n"),
      /BSC Groth16 transcript template package sha256 must match handoff reference/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("handoff-bundle rejects adversarial package schema drift", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-handoff-schema-"));
  try {
    const { inputs, result } = await writeAttestationRequestCandidate(root, {
      evidence: false,
    });
    const snarkjsBin = await copyExecutableFixture(
      inputs.snarkjsStub,
      join(root, "snarkjs-bin"),
    );
    const circomBin = await copyExecutableFixture(
      inputs.snarkjsStub,
      join(root, "circom-bin"),
    );
    const transcriptPackage = await writeBscGroth16TranscriptTemplates({
      "bsc-network": "testnet",
      r1cs: inputs.r1cs,
      zkey: inputs.zkey,
      ptau: inputs.ptau,
      "snarkjs-verifier-key": inputs.verificationKeyPath,
      "circuit-source": inputs.circuitSource,
      "witness-wasm": inputs.witnessWasm,
      "circom-bin": circomBin,
      "snarkjs-bin": snarkjsBin,
      "out-dir": join(root, "transcripts"),
    });
    const forgedPackage = JSON.parse(await readFile(transcriptPackage.out, "utf8"));
    forgedPackage.schema = BSC_GROTH16_EVIDENCE_TEMPLATE_PACKAGE_SCHEMA;
    await writeJson(transcriptPackage.out, forgedPackage);

    await assert.rejects(
      () =>
        writeBscGroth16AttestationHandoff({
          manifest: result.manifest,
          "transcript-template-package": transcriptPackage.out,
          out: join(root, "handoff.json"),
        }),
      /BSC Groth16 transcript template package schema must be iroha-sccp-bsc-groth16-transcript-template-package\/v1/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-request blocks semantic and circuit roles without review evidence", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-request-no-evidence-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root, {
      evidence: false,
    });
    const requestPath = join(root, "request.json");
    const privateKeyPath = await writePrivateKeyPem(join(root, "semantic-key.pem"));

    const requestResult = await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      "--toolchain-sha256",
      `0x${"ef".repeat(32)}`,
      "--out",
      requestPath,
    ]);
    const request = JSON.parse(await readFile(requestPath, "utf8"));

    assert.deepEqual(requestResult.readyForSignature, {
      semanticSccpCircuit: false,
      circuitSecurity: false,
      trustedSetup: true,
      reproducibleBuild: true,
    });
    assert.match(
      request.roles.semanticSccpCircuit.blockers.join("\n"),
      /missing semantic SCCP circuit review evidence artifact/u,
    );
    assert.match(
      request.roles.circuitSecurity.blockers.join("\n"),
      /missing circuit security audit evidence artifact/u,
    );
    assert.equal(request.evidence.semanticReview, null);
    assert.equal(request.evidence.circuitSecurityAudit, null);
    await assert.rejects(
      () =>
        signBscGroth16AttestationRole({
          request: requestPath,
          role: "semantic",
          "private-key-pem": privateKeyPath,
          out: join(root, "semantic-attestation.json"),
        }),
      /semantic SCCP circuit role is not ready for signature: missing semantic SCCP circuit review evidence artifact/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-request blocks malformed and adversarial review evidence", async () => {
  const cases = [
    {
      name: "semantic-false-coverage",
      role: "semanticSccpCircuit",
      evidenceOverrides: { semantic: { negativeCaseCoverage: false } },
      pattern: /negativeCaseCoverage must be true/u,
    },
    {
      name: "semantic-material-drift",
      role: "semanticSccpCircuit",
      evidenceOverrides: { semantic: { r1csSha256: `0x${"aa".repeat(32)}` } },
      pattern: /r1csSha256 must match/u,
    },
    {
      name: "semantic-route-alias-conflict",
      role: "semanticSccpCircuit",
      evidenceOverrides: { semantic: { route_id: "taira_bsc_xor" } },
      pattern: /semantic SCCP circuit review evidence routeId must not use multiple aliases: routeId, route_id/u,
    },
    {
      name: "semantic-shadow-field",
      role: "semanticSccpCircuit",
      evidenceOverrides: { semantic: { operatorOverrideApproved: true } },
      pattern: /semantic SCCP circuit review evidence contains unknown field: operatorOverrideApproved/u,
    },
    {
      name: "semantic-secret",
      role: "semanticSccpCircuit",
      evidenceOverrides: { semantic: { privateKey: "not-for-public-release" } },
      pattern: /privateKey|private key|secret/iu,
    },
    {
      name: "semantic-duplicate-key",
      role: "semanticSccpCircuit",
      mutate: async ({ result }) => {
        await writeFile(
          result.semanticReviewEvidence,
          `{"schema":"${BSC_GROTH16_SEMANTIC_REVIEW_EVIDENCE_SCHEMA}","schema":"${BSC_GROTH16_SEMANTIC_REVIEW_EVIDENCE_SCHEMA}"}\n`,
        );
      },
      pattern: /duplicate JSON object key/u,
    },
    {
      name: "semantic-report-drift",
      role: "semanticSccpCircuit",
      mutate: async ({ evidence }) => {
        await writeFile(evidence.semanticReport, "Changed semantic review report.\n");
      },
      pattern: /report sha256 must match/u,
    },
    {
      name: "semantic-report-shadow-field",
      role: "semanticSccpCircuit",
      mutate: async ({ evidence }) => {
        const record = JSON.parse(await readFile(evidence.semanticReviewEvidence, "utf8"));
        record.reviewReport.operatorNote = "approved outside signed role body";
        await writeJson(evidence.semanticReviewEvidence, record);
      },
      pattern: /semantic SCCP circuit review evidence report contains unknown field: operatorNote/u,
    },
    {
      name: "semantic-report-container-alias-conflict",
      role: "semanticSccpCircuit",
      mutate: async ({ evidence }) => {
        const record = JSON.parse(await readFile(evidence.semanticReviewEvidence, "utf8"));
        record.review_report = record.reviewReport;
        await writeJson(evidence.semanticReviewEvidence, record);
      },
      pattern: /semantic SCCP circuit review evidence reviewReport must not use multiple aliases: reviewReport, review_report/u,
    },
    {
      name: "semantic-report-path-alias-conflict",
      role: "semanticSccpCircuit",
      mutate: async ({ evidence }) => {
        const record = JSON.parse(await readFile(evidence.semanticReviewEvidence, "utf8"));
        record.reviewReport.reportPath = "semantic-review-report.md";
        await writeJson(evidence.semanticReviewEvidence, record);
      },
      pattern: /semantic SCCP circuit review evidence report path must not use multiple aliases: path, reportPath/u,
    },
    {
      name: "semantic-report-hash-alias-conflict",
      role: "semanticSccpCircuit",
      mutate: async ({ evidence }) => {
        const record = JSON.parse(await readFile(evidence.semanticReviewEvidence, "utf8"));
        record.reviewReport.hash = record.reviewReport.sha256;
        await writeJson(evidence.semanticReviewEvidence, record);
      },
      pattern: /semantic SCCP circuit review evidence report sha256 must not use multiple aliases: sha256, hash/u,
    },
    {
      name: "semantic-absolute-report-path",
      role: "semanticSccpCircuit",
      mutate: async ({ evidence }) => {
        const record = JSON.parse(await readFile(evidence.semanticReviewEvidence, "utf8"));
        record.reviewReport.path = evidence.semanticReport;
        await writeJson(evidence.semanticReviewEvidence, record);
      },
      pattern: /report path must be a safe relative path/u,
    },
    {
      name: "semantic-url-report-path",
      role: "semanticSccpCircuit",
      mutate: async ({ evidence }) => {
        const record = JSON.parse(await readFile(evidence.semanticReviewEvidence, "utf8"));
        record.reviewReport.path = "https://reviews.example.invalid/semantic-report.md";
        await writeJson(evidence.semanticReviewEvidence, record);
      },
      pattern: /report path must be a safe relative path/u,
    },
    {
      name: "semantic-parent-report-path",
      role: "semanticSccpCircuit",
      mutate: async ({ evidence }) => {
        const record = JSON.parse(await readFile(evidence.semanticReviewEvidence, "utf8"));
        record.reviewReport.path = "%2e%2e/semantic-review-report.md";
        await writeJson(evidence.semanticReviewEvidence, record);
      },
      pattern: /report path must be a safe relative path/u,
    },
    {
      name: "semantic-placeholder-report-path",
      role: "semanticSccpCircuit",
      mutate: async ({ evidence }) => {
        const placeholderReport = join(
          dirname(evidence.semanticReviewEvidence),
          "placeholder-semantic-review-report.md",
        );
        await writeFile(placeholderReport, await readFile(evidence.semanticReport));
        const record = JSON.parse(await readFile(evidence.semanticReviewEvidence, "utf8"));
        record.reviewReport.path = "placeholder-semantic-review-report.md";
        record.reviewReport.sha256 = sha256Hex(await readFile(placeholderReport));
        await writeJson(evidence.semanticReviewEvidence, record);
      },
      pattern: /must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
    },
    {
      name: "semantic-symlink-report-path",
      role: "semanticSccpCircuit",
      mutate: async ({ evidence }) => {
        const linkPath = join(dirname(evidence.semanticReviewEvidence), "semantic-review-link.md");
        await symlink(evidence.semanticReport, linkPath);
        const record = JSON.parse(await readFile(evidence.semanticReviewEvidence, "utf8"));
        record.reviewReport.path = "semantic-review-link.md";
        record.reviewReport.sha256 = sha256Hex(await readFile(evidence.semanticReport));
        await writeJson(evidence.semanticReviewEvidence, record);
      },
      pattern: /must not be a symbolic link/u,
    },
    {
      name: "semantic-oversized-report-path",
      role: "semanticSccpCircuit",
      mutate: async ({ evidence }) => {
        const largeReport = join(dirname(evidence.semanticReviewEvidence), "semantic-review-large.md");
        await writeFile(largeReport, Buffer.alloc(17 * 1024 * 1024, 0x61));
        const record = JSON.parse(await readFile(evidence.semanticReviewEvidence, "utf8"));
        record.reviewReport.path = "semantic-review-large.md";
        record.reviewReport.sha256 = sha256Hex(await readFile(largeReport));
        await writeJson(evidence.semanticReviewEvidence, record);
      },
      pattern: /maximum allowed/u,
    },
    {
      name: "circuit-high-finding",
      role: "circuitSecurity",
      evidenceOverrides: { security: { highFindings: 1 } },
      pattern: /highFindings must be 0/u,
    },
    {
      name: "circuit-shadow-field",
      role: "circuitSecurity",
      evidenceOverrides: { security: { securityCommitteeOverride: true } },
      pattern: /circuit security audit evidence contains unknown field: securityCommitteeOverride/u,
    },
    {
      name: "circuit-approved-alias-conflict",
      role: "circuitSecurity",
      evidenceOverrides: { security: { production_approved: true } },
      pattern: /circuit security audit evidence productionApproved must not use multiple aliases: production_approved, approved/u,
    },
    {
      name: "circuit-placeholder-label",
      role: "circuitSecurity",
      evidenceOverrides: { security: { auditId: "placeholder-audit" } },
      pattern: /must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
    },
    {
      name: "circuit-report-drift",
      role: "circuitSecurity",
      mutate: async ({ evidence }) => {
        await writeFile(evidence.circuitReport, "Changed circuit audit report.\n");
      },
      pattern: /report sha256 must match/u,
    },
    {
      name: "circuit-report-shadow-field",
      role: "circuitSecurity",
      mutate: async ({ evidence }) => {
        const record = JSON.parse(await readFile(evidence.circuitSecurityAuditEvidence, "utf8"));
        record.auditReport.operatorNote = "approved outside signed role body";
        await writeJson(evidence.circuitSecurityAuditEvidence, record);
      },
      pattern: /circuit security audit evidence report contains unknown field: operatorNote/u,
    },
    {
      name: "circuit-absolute-report-path",
      role: "circuitSecurity",
      mutate: async ({ evidence }) => {
        const record = JSON.parse(await readFile(evidence.circuitSecurityAuditEvidence, "utf8"));
        record.auditReport.path = evidence.circuitReport;
        await writeJson(evidence.circuitSecurityAuditEvidence, record);
      },
      pattern: /report path must be a safe relative path/u,
    },
    {
      name: "circuit-parent-report-path",
      role: "circuitSecurity",
      mutate: async ({ evidence }) => {
        const record = JSON.parse(await readFile(evidence.circuitSecurityAuditEvidence, "utf8"));
        record.auditReport.path = "%2e%2e/circuit-security-audit-report.md";
        await writeJson(evidence.circuitSecurityAuditEvidence, record);
      },
      pattern: /report path must be a safe relative path/u,
    },
    {
      name: "circuit-symlink-report-path",
      role: "circuitSecurity",
      mutate: async ({ evidence }) => {
        const linkPath = join(dirname(evidence.circuitSecurityAuditEvidence), "circuit-audit-link.md");
        await symlink(evidence.circuitReport, linkPath);
        const record = JSON.parse(await readFile(evidence.circuitSecurityAuditEvidence, "utf8"));
        record.auditReport.path = "circuit-audit-link.md";
        record.auditReport.sha256 = sha256Hex(await readFile(evidence.circuitReport));
        await writeJson(evidence.circuitSecurityAuditEvidence, record);
      },
      pattern: /must not be a symbolic link/u,
    },
    {
      name: "circuit-oversized-report-path",
      role: "circuitSecurity",
      mutate: async ({ evidence }) => {
        const largeReport = join(dirname(evidence.circuitSecurityAuditEvidence), "circuit-audit-large.md");
        await writeFile(largeReport, Buffer.alloc(17 * 1024 * 1024, 0x63));
        const record = JSON.parse(await readFile(evidence.circuitSecurityAuditEvidence, "utf8"));
        record.auditReport.path = "circuit-audit-large.md";
        record.auditReport.sha256 = sha256Hex(await readFile(largeReport));
        await writeJson(evidence.circuitSecurityAuditEvidence, record);
      },
      pattern: /maximum allowed/u,
    },
  ];

  for (const testCase of cases) {
    const root = await mkdtemp(
      join(tmpdir(), `iroha-bsc-groth16-evidence-${testCase.name}-`),
    );
    try {
      const candidate = await writeAttestationRequestCandidate(root, {
        evidenceOverrides: testCase.evidenceOverrides ?? {},
      });
      if (testCase.mutate) {
        await testCase.mutate(candidate);
      }
      const requestPath = join(root, "request.json");
      const result = await main([
        "attestation-request",
        "--manifest",
        candidate.result.manifest,
        ...candidate.result.attestationRequestEvidenceArgs,
        "--toolchain-sha256",
        `0x${"ef".repeat(32)}`,
        "--out",
        requestPath,
      ]);
      const request = JSON.parse(await readFile(requestPath, "utf8"));

      assert.equal(result.readyForSignature[testCase.role], false, testCase.name);
      assert.match(
        request.roles[testCase.role].blockers.join("\n"),
        testCase.pattern,
        testCase.name,
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  }
});

test("attestation-request rejects unsafe review evidence artifact option paths", async () => {
  const unsafePaths = [
    {
      name: "uri",
      path: "https://reviews.example.invalid/semantic-review-evidence.json",
    },
    {
      name: "query",
      path: "semantic-review-evidence.json?sha256=abc",
    },
    {
      name: "fragment",
      path: "semantic-review-evidence.json#approved",
    },
    {
      name: "backslash",
      path: "review\\semantic-review-evidence.json",
    },
    {
      name: "encoded-parent",
      path: "%2e%2e/semantic-review-evidence.json",
    },
    {
      name: "encoded-separator",
      path: "review%2fsemantic-review-evidence.json",
    },
    {
      name: "current-segment",
      path: "./semantic-review-evidence.json",
    },
    {
      name: "empty-segment",
      path: "review//semantic-review-evidence.json",
    },
    {
      name: "nul-byte",
      path: "semantic-review-evidence.json\0shadow",
    },
  ];
  const roles = [
    {
      flag: "--semantic-review-evidence",
      role: "semanticSccpCircuit",
      evidenceKey: "semanticReview",
      label: "semantic SCCP circuit review",
    },
    {
      flag: "--circuit-security-audit-evidence",
      role: "circuitSecurity",
      evidenceKey: "circuitSecurityAudit",
      label: "circuit security audit",
    },
  ];

  for (const roleCase of roles) {
    for (const pathCase of unsafePaths) {
      const root = await mkdtemp(
        join(
          tmpdir(),
          `iroha-bsc-groth16-evidence-path-${roleCase.role}-${pathCase.name}-`,
        ),
      );
      try {
        const candidate = await writeAttestationRequestCandidate(root);
        const args = [...candidate.result.attestationRequestEvidenceArgs];
        const flagIndex = args.indexOf(roleCase.flag);
        assert.notEqual(flagIndex, -1, roleCase.flag);
        args[flagIndex + 1] = pathCase.path;
        const requestPath = join(root, "request.json");

        const result = await main([
          "attestation-request",
          "--manifest",
          candidate.result.manifest,
          ...args,
          "--toolchain-sha256",
          `0x${"ef".repeat(32)}`,
          "--out",
          requestPath,
        ]);
        const request = JSON.parse(await readFile(requestPath, "utf8"));

        assert.equal(
          result.readyForSignature[roleCase.role],
          false,
          `${roleCase.role} ${pathCase.name}`,
        );
        assert.equal(request.evidence[roleCase.evidenceKey], null);
        assert.match(
          request.roles[roleCase.role].blockers.join("\n"),
          new RegExp(
            `${roleCase.label} evidence artifact path must be a safe local artifact path`,
            "u",
          ),
          `${roleCase.role} ${pathCase.name}`,
        );
      } finally {
        await rm(root, { recursive: true, force: true });
      }
    }
  }
});

test("sign-attestation refuses tampered evidence references even when role hashes are refreshed", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-evidence-tamper-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    const privateKeyPath = await writePrivateKeyPem(join(root, "semantic-key.pem"));
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      result.reproducibleBuildToolchainSha256,
      "--out",
      requestPath,
    ]);
    const request = JSON.parse(await readFile(requestPath, "utf8"));
    request.evidence.semanticReview.sha256 = `0x${"77".repeat(32)}`;
    const role = request.roles.semanticSccpCircuit;
    const bodyHash = sha256Hex(canonicalJson(role.body));
    role.signedPayloadSha256 = bodyHash;
    role.signatureTemplate.signedPayloadSha256 = bodyHash;
    await writeJson(requestPath, request);

    await assert.rejects(
      () =>
        signBscGroth16AttestationRole({
          request: requestPath,
          role: "semantic",
          "private-key-pem": privateKeyPath,
          out: join(root, "semantic-attestation.json"),
        }),
      /body evidence sha256 must match evidence reference/u,
    );
    const status = await auditBscGroth16AttestationStatus({
      request: requestPath,
      ...trustedSignerOption(),
    });
    assert.equal(status.requestReadyForSignature.semanticSccpCircuit, false);
    assert.match(
      status.roles.semanticSccpCircuit.blockers.join("\n"),
      /body evidence sha256 must match evidence reference/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-request refuses manifests without transcript artifacts", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-request-missing-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    delete manifest.artifacts.trustedSetupTranscript;
    await writeJson(result.manifest, manifest);

    await assert.rejects(
      () =>
        main([
          "attestation-request",
          "--manifest",
          result.manifest,
          ...result.attestationRequestEvidenceArgs,
          "--toolchain-sha256",
          `0x${"ef".repeat(32)}`,
          "--out",
          join(root, "request.json"),
        ]),
      /trustedSetupTranscript artifact is required/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-request refuses transcript hash drift", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-request-drift-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    await writeJson(manifest.artifacts.trustedSetupTranscript.path, {
      schema: "iroha-sccp-bsc-trusted-setup-transcript/test-fixture",
      contributors: ["fixture-contributor-a", "fixture-contributor-b"],
      toxicWasteDestroyed: true,
      postManifestMutation: true,
    });

    await assert.rejects(
      () =>
        main([
          "attestation-request",
          "--manifest",
          result.manifest,
          ...result.attestationRequestEvidenceArgs,
          "--toolchain-sha256",
          `0x${"ef".repeat(32)}`,
          "--out",
          join(root, "request.json"),
        ]),
      /trusted setup transcript sha256 must match material manifest/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-request refuses manifest public signal drift", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-request-signals-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    manifest.publicSignalNames = manifest.publicSignalNames.slice(0, 8);
    await writeJson(result.manifest, manifest);

    await assert.rejects(
      () =>
        main([
          "attestation-request",
          "--manifest",
          result.manifest,
          ...result.attestationRequestEvidenceArgs,
          "--toolchain-sha256",
          `0x${"ef".repeat(32)}`,
          "--out",
          join(root, "request.json"),
        ]),
      /publicSignalNames must match BSC Groth16 public signals/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-request refuses unsafe material manifest artifact paths", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-request-artifact-paths-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const baseline = JSON.parse(await readFile(result.manifest, "utf8"));
    const cases = [
      {
        name: "url",
        path: "https://example.invalid/sccp-bsc-full-message-v1.r1cs",
      },
      {
        name: "query",
        path: "sccp-bsc-full-message-v1.r1cs?sha256=abc",
      },
      {
        name: "backslash",
        path: "material\\sccp-bsc-full-message-v1.r1cs",
      },
      {
        name: "encoded-parent",
        path: "%2e%2e/sccp-bsc-full-message-v1.r1cs",
      },
      {
        name: "double-encoded-parent",
        path: "%252e%252e/sccp-bsc-full-message-v1.r1cs",
      },
      {
        name: "encoded-separator",
        path: "material%2fsccp-bsc-full-message-v1.r1cs",
      },
      {
        name: "current-segment",
        path: "./sccp-bsc-full-message-v1.r1cs",
      },
      {
        name: "empty-segment",
        path: "material//sccp-bsc-full-message-v1.r1cs",
      },
    ];

    for (const testCase of cases) {
      await writeJson(result.manifest, {
        ...baseline,
        artifacts: {
          ...baseline.artifacts,
          r1cs: {
            ...baseline.artifacts.r1cs,
            path: testCase.path,
          },
        },
      });

      await assert.rejects(
        () =>
          main([
            "attestation-request",
            "--manifest",
            result.manifest,
            ...result.attestationRequestEvidenceArgs,
            "--toolchain-sha256",
            `0x${"ef".repeat(32)}`,
            "--out",
            join(root, `request.${testCase.name}.json`),
          ]),
        /material manifest artifacts\.r1cs\.path must be a safe artifact path/u,
        testCase.name,
      );
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-request refuses material manifest shadow fields and aliases", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-request-shape-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const baseline = JSON.parse(await readFile(result.manifest, "utf8"));
    await writeJson(result.manifest, {
      ...baseline,
      operatorShadow: true,
      artifacts: {
        ...baseline.artifacts,
        r1cs: {
          ...baseline.artifacts.r1cs,
          shadowHash: `0x${"11".repeat(32)}`,
        },
      },
      selfChecks: {
        ...baseline.selfChecks,
        snarkjs: {
          ...baseline.selfChecks.snarkjs,
          remoteProver: true,
        },
      },
      attestationTrustPolicy: {
        ...baseline.attestationTrustPolicy,
        operatorOverride: true,
      },
      attestations: {
        ...baseline.attestations,
        shadowRole: {
          path: "shadow-attestation.json",
          sha256: `0x${"22".repeat(32)}`,
        },
      },
    });

    await assert.rejects(
      () =>
        main([
          "attestation-request",
          "--manifest",
          result.manifest,
          ...result.attestationRequestEvidenceArgs,
          "--toolchain-sha256",
          `0x${"ef".repeat(32)}`,
          "--out",
          join(root, "request.shadow.json"),
        ]),
      (error) => {
        const message = String(error);
        assert.match(
          message,
          /material manifest shape is not production-ready/u,
        );
        assert.match(message, /material manifest contains unknown field: operatorShadow/u);
        assert.match(message, /material manifest artifacts\.r1cs contains unknown field: shadowHash/u);
        assert.match(message, /material manifest selfChecks\.snarkjs contains unknown field: remoteProver/u);
        assert.match(
          message,
          /material manifest attestationTrustPolicy contains unknown field: operatorOverride/u,
        );
        assert.match(message, /material manifest attestations contains unknown field: shadowRole/u);
        return true;
      },
    );

    await writeJson(result.manifest, {
      ...baseline,
      route_id: baseline.routeId,
      public_signal_names: baseline.publicSignalNames,
      proof_artifact_hash: baseline.proofArtifactHash,
      proving_key_hash: baseline.provingKeyHash,
      production_ready: baseline.productionReady,
      artifacts: {
        ...baseline.artifacts,
        powers_of_tau: baseline.artifacts.powersOfTau,
        provingKey: {
          ...baseline.artifacts.provingKey,
          artifactHash: baseline.artifacts.provingKey.sha256,
        },
      },
      self_checks: baseline.selfChecks,
      attestation_trust_policy: baseline.attestationTrustPolicy,
    });

    await assert.rejects(
      () =>
        main([
          "attestation-request",
          "--manifest",
          result.manifest,
          ...result.attestationRequestEvidenceArgs,
          "--toolchain-sha256",
          `0x${"ef".repeat(32)}`,
          "--out",
          join(root, "request.alias.json"),
        ]),
      (error) => {
        const message = String(error);
        assert.match(message, /material manifest routeId must not use multiple aliases: routeId, route_id/u);
        assert.match(
          message,
          /material manifest publicSignalNames must not use multiple aliases: publicSignalNames, public_signal_names/u,
        );
        assert.match(
          message,
          /material manifest productionReady must not use multiple aliases: productionReady, production_ready/u,
        );
        assert.match(
          message,
          /material manifest proofArtifactHash must not use multiple aliases: proofArtifactHash, proof_artifact_hash/u,
        );
        assert.match(
          message,
          /material manifest provingKeyHash must not use multiple aliases: provingKeyHash, proving_key_hash/u,
        );
        assert.match(
          message,
          /material manifest artifacts powersOfTau must not use multiple aliases: powersOfTau, powers_of_tau/u,
        );
        assert.match(
          message,
          /material manifest artifacts\.provingKey sha256 must not use multiple aliases: sha256, artifactHash/u,
        );
        assert.match(message, /material manifest selfChecks must not use multiple aliases: selfChecks, self_checks/u);
        assert.match(
          message,
          /material manifest attestationTrustPolicy must not use multiple aliases: attestationTrustPolicy, attestation_trust_policy/u,
        );
        return true;
      },
    );

    await writeJson(result.manifest, {
      ...baseline,
      proofArtifactHash: `0x${"33".repeat(32)}`,
    });

    await assert.rejects(
      () =>
        main([
          "attestation-request",
          "--manifest",
          result.manifest,
          ...result.attestationRequestEvidenceArgs,
          "--toolchain-sha256",
          `0x${"ef".repeat(32)}`,
          "--out",
          join(root, "request.proof-hash-drift.json"),
        ]),
      /material manifest proofArtifactHash must match artifacts\.r1cs\.sha256/u,
    );

    await writeJson(result.manifest, {
      ...baseline,
      provingKeyHash: `0x${"44".repeat(32)}`,
    });

    await assert.rejects(
      () =>
        main([
          "attestation-request",
          "--manifest",
          result.manifest,
          ...result.attestationRequestEvidenceArgs,
          "--toolchain-sha256",
          `0x${"ef".repeat(32)}`,
          "--out",
          join(root, "request.proving-hash-drift.json"),
        ]),
      /material manifest provingKeyHash must match artifacts\.provingKey\.sha256/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-request requires a reproducible-build toolchain hash source", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-request-toolchain-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root, {
      materialInputs: { includeToolchain: false },
    });

    await assert.rejects(
      () =>
        main([
          "attestation-request",
          "--manifest",
          result.manifest,
          ...result.attestationRequestEvidenceArgs,
          "--out",
          join(root, "request.json"),
        ]),
      /toolchain object is required to derive toolchainSha256/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("sign-attestation writes one request-bound Ed25519 role attestation", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-sign-role-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    const out = join(root, "semantic-attestation.json");
    const privateKeyPath = await writePrivateKeyPem(join(root, "semantic-key.pem"));
    const requestResult = await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      result.reproducibleBuildToolchainSha256,
      "--out",
      requestPath,
    ]);

    const signed = await main([
      "sign-attestation",
      "--request",
      requestPath,
      "--role",
      "semanticSccpCircuit",
      "--private-key-pem",
      privateKeyPath,
      "--signer-fingerprint",
      TEST_ATTESTATION_SIGNER_FINGERPRINT,
      "--out",
      out,
    ]);
    const record = JSON.parse(await readFile(out, "utf8"));
    const { signature: _signature, signatures: _signatures, ...body } = record;

    assert.equal(signed.ok, true);
    assert.equal(signed.role, "semanticSccpCircuit");
    assert.equal(signed.signerFingerprint, TEST_ATTESTATION_SIGNER_FINGERPRINT);
    assert.equal(signed.signedPayloadSha256, requestResult.signedPayloadSha256.semanticSccpCircuit);
    assert.equal(signed.attestationSha256, sha256Hex(await readFile(out)));
    assert.deepEqual(body, JSON.parse(await readFile(requestPath, "utf8")).roles.semanticSccpCircuit.body);
    assert.deepEqual(record.signature, {
      schema: BSC_GROTH16_ATTESTATION_SIGNATURE_SCHEMA,
      algorithm: "ed25519",
      signerFingerprint: TEST_ATTESTATION_SIGNER_FINGERPRINT,
      publicKeyPem: TEST_ATTESTATION_PUBLIC_KEY_PEM,
      signedPayloadSha256: requestResult.signedPayloadSha256.semanticSccpCircuit,
      signature: record.signature.signature,
    });
    assert.equal(
      record.signature.signedPayloadSha256,
      sha256Hex(Buffer.from(canonicalJson(body), "utf8")),
    );
    assert.match(record.signature.signature, /^[A-Za-z0-9+/]+={0,2}$/u);
    assert.doesNotMatch(JSON.stringify(record), /PRIVATE KEY|privateKey|private_key/u);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("sign-attestation refuses blocked request roles", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-sign-blocked-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    const privateKeyPath = await writePrivateKeyPem(join(root, "setup-key.pem"), SETUP_ATTESTATION_SIGNER);
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      result.reproducibleBuildToolchainSha256,
      "--out",
      requestPath,
    ]);
    const request = JSON.parse(await readFile(requestPath, "utf8"));
    request.roles.trustedSetup.readyForSignature = false;
    request.roles.trustedSetup.blockers = ["ceremony transcript has not passed"];
    await writeJson(requestPath, request);

    await assert.rejects(
      () =>
        signBscGroth16AttestationRole({
          request: requestPath,
          role: "trustedSetup",
          "private-key-pem": privateKeyPath,
          out: join(root, "setup-attestation.json"),
        }),
      /trusted setup role is not ready for signature: ceremony transcript has not passed/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("sign-attestation refuses tampered request body and template hashes", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-sign-tampered-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    const privateKeyPath = await writePrivateKeyPem(join(root, "semantic-key.pem"));
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      result.reproducibleBuildToolchainSha256,
      "--out",
      requestPath,
    ]);
    const request = JSON.parse(await readFile(requestPath, "utf8"));
    request.roles.semanticSccpCircuit.body.negativeCaseCoverage = false;
    await writeJson(requestPath, request);

    await assert.rejects(
      () =>
        signBscGroth16AttestationRole({
          request: requestPath,
          role: "semantic",
          "private-key-pem": privateKeyPath,
          out: join(root, "semantic-attestation.json"),
        }),
      /semantic SCCP circuit signedPayloadSha256 must match role body/u,
    );

    request.roles.semanticSccpCircuit.body.negativeCaseCoverage = true;
    request.roles.semanticSccpCircuit.signatureTemplate.signedPayloadSha256 =
      `0x${"44".repeat(32)}`;
    await writeJson(requestPath, request);
    await assert.rejects(
      () =>
        signBscGroth16AttestationRole({
          request: requestPath,
          role: "semantic",
          "private-key-pem": privateKeyPath,
          out: join(root, "semantic-attestation.json"),
        }),
      /semantic SCCP circuit signatureTemplate signedPayloadSha256 must match role body/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("sign-attestation refuses request role bodies with unknown shadow fields", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-sign-shadow-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    const privateKeyPath = await writePrivateKeyPem(join(root, "semantic-key.pem"));
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      result.reproducibleBuildToolchainSha256,
      "--out",
      requestPath,
    ]);
    const request = JSON.parse(await readFile(requestPath, "utf8"));
    const role = request.roles.semanticSccpCircuit;
    role.body.semanticShadowDecision = true;
    const bodyHash = sha256Hex(canonicalJson(role.body));
    role.signedPayloadSha256 = bodyHash;
    role.signatureTemplate.signedPayloadSha256 = bodyHash;
    await writeJson(requestPath, request);

    await assert.rejects(
      () =>
        signBscGroth16AttestationRole({
          request: requestPath,
          role: "semantic",
          "private-key-pem": privateKeyPath,
          out: join(root, "semantic-attestation.json"),
        }),
      /attestation request package semantic SCCP circuit body contains unknown field: semanticShadowDecision/u,
    );

    const status = await auditBscGroth16AttestationStatus({
      request: requestPath,
      "trusted-attestation-signer": TRUSTED_ATTESTATION_SIGNER_FINGERPRINTS.join(
        ",",
      ),
    });
    assert.equal(status.readyToFinalize, false);
    assert.equal(status.requestReadyForSignature.semanticSccpCircuit, false);
    assert.match(
      status.roles.semanticSccpCircuit.blockers.join("\n"),
      /attestation request package semantic SCCP circuit body contains unknown field: semanticShadowDecision/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("sign-attestation refuses request package shadow fields and duplicate aliases", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-request-shape-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    const privateKeyPath = await writePrivateKeyPem(join(root, "semantic-key.pem"));
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      result.reproducibleBuildToolchainSha256,
      "--out",
      requestPath,
    ]);
    const request = JSON.parse(await readFile(requestPath, "utf8"));
    request.operatorShadowDecision = true;
    request.manifest.production_ready = request.manifest.productionReady;
    request.artifacts.r1cs.artifactHash = request.artifacts.r1cs.sha256;
    request.evidence.semanticReview.operatorOverride = true;
    request.evidence.semanticReview.report.reportPath =
      request.evidence.semanticReview.report.path;
    request.evidenceValidation.semanticReview.hash =
      request.evidenceValidation.semanticReview.sha256;
    request.transcriptValidation.trustedSetup.hash =
      request.transcriptValidation.trustedSetup.sha256;
    request.roles.circuitSecurity.operatorOverrideApproved = true;
    request.roles.semanticSccpCircuit.signatureTemplate.signed_payload_sha256 =
      request.roles.semanticSccpCircuit.signatureTemplate.signedPayloadSha256;
    request.signingInstructions.operatorPolicy = "sign anyway";
    await writeJson(requestPath, request);

    let message = "";
    await assert.rejects(
      async () =>
        signBscGroth16AttestationRole({
          request: requestPath,
          role: "semantic",
          "private-key-pem": privateKeyPath,
          out: join(root, "semantic-attestation.json"),
        }),
      (error) => {
        message = error instanceof Error ? error.message : String(error);
        return /attestation request package shape is not production-ready/u.test(
          message,
        );
      },
    );
    assert.match(
      message,
      /attestation request package contains unknown field: operatorShadowDecision/u,
    );
    assert.match(
      message,
      /attestation request package manifest productionReady must not use multiple aliases: productionReady, production_ready/u,
    );
    assert.match(
      message,
      /attestation request package artifacts\.r1cs sha256 must not use multiple aliases: sha256, artifactHash/u,
    );
    assert.match(
      message,
      /attestation request package evidence\.semanticReview contains unknown field: operatorOverride/u,
    );
    assert.match(
      message,
      /attestation request package evidence\.semanticReview\.report path must not use multiple aliases: path, reportPath/u,
    );
    assert.match(
      message,
      /attestation request package evidenceValidation\.semanticReview sha256 must not use multiple aliases: sha256, hash/u,
    );
    assert.match(
      message,
      /attestation request package transcriptValidation\.trustedSetup sha256 must not use multiple aliases: sha256, hash/u,
    );
    assert.match(
      message,
      /attestation request package circuit security role contains unknown field: operatorOverrideApproved/u,
    );
    assert.match(
      message,
      /attestation request package semantic SCCP circuit signatureTemplate signedPayloadSha256 must not use multiple aliases: signedPayloadSha256, signed_payload_sha256/u,
    );
    assert.match(
      message,
      /attestation request package signingInstructions contains unknown field: operatorPolicy/u,
    );

    const status = await auditBscGroth16AttestationStatus({
      request: requestPath,
      "trusted-attestation-signer": TRUSTED_ATTESTATION_SIGNER_FINGERPRINTS.join(
        ",",
      ),
    });
    assert.equal(status.readyToFinalize, false);
    assert.equal(status.requestValid, false);
    assert.match(
      status.problems.join("\n"),
      /attestation request package shape is not production-ready/u,
    );
    assert.doesNotMatch(
      JSON.stringify(status),
      /PRIVATE KEY|privateKey|private_key|mnemonic|seed phrase|password/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("sign-attestation refuses mismatched fingerprints, non-Ed25519 keys, and key overwrite outputs", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-sign-key-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    const privateKeyPath = await writePrivateKeyPem(join(root, "semantic-key.pem"));
    const wrongKeyPath = join(root, "rsa-key.pem");
    const { privateKey: rsaPrivateKey } = generateKeyPairSync("rsa", {
      modulusLength: 2048,
    });
    await writeFile(
      wrongKeyPath,
      rsaPrivateKey.export({ type: "pkcs8", format: "pem" }),
      { mode: 0o600 },
    );
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      result.reproducibleBuildToolchainSha256,
      "--out",
      requestPath,
    ]);

    await assert.rejects(
      () =>
        signBscGroth16AttestationRole({
          request: requestPath,
          role: "semantic",
          "private-key-pem": privateKeyPath,
          "signer-fingerprint": UNTRUSTED_ATTESTATION_SIGNER_FINGERPRINT,
          out: join(root, "semantic-attestation.json"),
        }),
      /signer fingerprint must match the supplied Ed25519 private key/u,
    );
    await assert.rejects(
      () =>
        signBscGroth16AttestationRole({
          request: requestPath,
          role: "semantic",
          "private-key-pem": wrongKeyPath,
          out: join(root, "semantic-attestation.json"),
        }),
      /private key must be ed25519/u,
    );
    await assert.rejects(
      () =>
        signBscGroth16AttestationRole({
          request: requestPath,
          role: "semantic",
          "private-key-pem": privateKeyPath,
          out: privateKeyPath,
        }),
      /output must not overwrite the private key file/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-status reports unsigned ready requests without finalizing", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-status-unsigned-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      `0x${"ef".repeat(32)}`,
      "--out",
      requestPath,
    ]);

    const status = await main([
      "attestation-status",
      "--request",
      requestPath,
      "--trusted-attestation-signer",
      TRUSTED_ATTESTATION_SIGNER_FINGERPRINTS.join(","),
    ]);

    assert.equal(status.readyToFinalize, false);
    assert.equal(status.requestValid, true);
    assert.deepEqual(status.requestReadyForSignature, {
      semanticSccpCircuit: true,
      circuitSecurity: true,
      trustedSetup: true,
      reproducibleBuild: true,
    });
    assert.deepEqual(
      status.missingSignedRoles.sort(),
      [
        "semanticSccpCircuit",
        "circuitSecurity",
        "trustedSetup",
        "reproducibleBuild",
      ].sort(),
    );
    assert.match(status.nextActions.join("\n"), /Sign the missing ready role payloads/u);
    assert.doesNotMatch(
      JSON.stringify(status),
      /PRIVATE KEY|privateKey|private_key|mnemonic|seed phrase|password/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-status accepts signer-produced role files as ready to finalize", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-status-ready-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      result.reproducibleBuildToolchainSha256,
      "--out",
      requestPath,
    ]);
    const attestations = await writeAttestationsFromRequest(root, requestPath);

    const status = await auditBscGroth16AttestationStatus({
      request: requestPath,
      ...trustedSignerOption(),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
    });

    assert.equal(status.readyToFinalize, true);
    assert.equal(status.problemCount, 0);
    assert.deepEqual(status.missingSignedRoles, []);
    assert.deepEqual(status.signedRoleProblems, []);
    assert.deepEqual(status.materialValidationBlockers, []);
    assert.equal(
      status.signedRoles.semanticSccpCircuit.signature.signerFingerprint,
      TEST_ATTESTATION_SIGNER_FINGERPRINT,
    );
    assert.equal(status.signedRoles.semanticSccpCircuit.signature.verified, true);
    assert.match(status.nextActions.join("\n"), /Run finalize-attestations/u);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-inventory selects exact request attestations and flags stale signed candidates", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-inventory-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      result.reproducibleBuildToolchainSha256,
      "--out",
      requestPath,
    ]);
    await writeAttestationsFromRequest(join(root, "current"), requestPath);
    await writeAttestationsFromRequest(join(root, "stale"), requestPath, {
      semantic: {
        negativeCaseCoverage: false,
      },
    });

    const inventory = await inventoryBscGroth16Attestations({
      request: requestPath,
      "scan-dir": root,
      ...trustedSignerOption(),
    });

    assert.equal(inventory.candidateSetReady, true);
    assert.equal(inventory.problemCount, 0);
    assert.equal(inventory.missingUsableRoles.length, 0);
    assert.equal(inventory.signerDiversityBlockers.length, 0);
    assert.ok(inventory.ignoredJsonCount > 0);
    assert.match(
      inventory.roleSummary.semanticSccpCircuit.selected.path,
      /current\/request-attestations\/semantic\.json$/u,
    );
    assert.equal(inventory.roleSummary.semanticSccpCircuit.usableCount, 1);
    assert.equal(
      inventory.roleSummary.semanticSccpCircuit.classifications[
        "stale-or-wrong-request"
      ],
      1,
    );
    const staleSemantic = inventory.candidates.find((candidate) =>
      candidate.path.endsWith("stale/request-attestations/semantic.json"),
    );
    assert.equal(staleSemantic.classification, "stale-or-wrong-request");
    assert.equal(staleSemantic.usable, false);
    assert.match(
      staleSemantic.problems.join("\n"),
      /signed attestation body does not match this request package/u,
    );
    assert.doesNotMatch(
      JSON.stringify(inventory),
      /PRIVATE KEY|privateKey|private_key|mnemonic|seed phrase|password/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-status flags blocked roles and supplied signatures for unready requests", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-status-blocked-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      `0x${"ef".repeat(32)}`,
      "--out",
      requestPath,
    ]);
    const request = JSON.parse(await readFile(requestPath, "utf8"));
    request.roles.trustedSetup.readyForSignature = false;
    request.roles.trustedSetup.blockers = [
      "operator did not publish ceremony transcript",
    ];
    await writeJson(requestPath, request);
    const attestations = await writeAttestationsFromRequest(root, requestPath);

    const status = await auditBscGroth16AttestationStatus({
      request: requestPath,
      ...trustedSignerOption(),
      "semantic-attestation": attestations.semantic,
      "trusted-setup-attestation": attestations.setup,
    });

    assert.equal(status.readyToFinalize, false);
    assert.equal(status.requestReadyForSignature.trustedSetup, false);
    assert.match(
      status.roles.trustedSetup.blockers.join("\n"),
      /operator did not publish ceremony transcript/u,
    );
    assert.match(
      status.signedRoleProblems.join("\n"),
      /trusted setup signed attestation was supplied, but the request role is not ready/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-status flags stale request bodies, forged signatures, and signer reuse", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-status-adversarial-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      `0x${"ef".repeat(32)}`,
      "--out",
      requestPath,
    ]);
    const staleRequestPath = join(root, "stale-request.json");
    const staleRequest = JSON.parse(await readFile(requestPath, "utf8"));
    staleRequest.roles.semanticSccpCircuit.body.negativeCaseCoverage = false;
    await writeJson(staleRequestPath, staleRequest);
    const staleStatus = await auditBscGroth16AttestationStatus({
      request: staleRequestPath,
      ...trustedSignerOption(),
    });
    assert.equal(staleStatus.readyToFinalize, false);
    assert.match(
      staleStatus.roles.semanticSccpCircuit.blockers.join("\n"),
      /signedPayloadSha256 must match role body/u,
    );

    const forged = await writeAttestationsFromRequest(root, requestPath, {
      semantic: {
        negativeCaseCoverage: false,
      },
      signingByRole: {
        security: defaultAttestationSigning().semantic,
        setup: defaultAttestationSigning().semantic,
        reproducible: defaultAttestationSigning().semantic,
      },
    });
    const forgedStatus = await auditBscGroth16AttestationStatus({
      request: requestPath,
      ...trustedSignerOption(),
      "semantic-attestation": forged.semantic,
      "circuit-security-attestation": forged.security,
      "trusted-setup-attestation": forged.setup,
      "reproducible-build-attestation": forged.reproducible,
    });

    assert.equal(forgedStatus.readyToFinalize, false);
    assert.match(
      forgedStatus.signedRoleProblems.join("\n"),
      /semantic SCCP circuit signed attestation body must match/u,
    );
    assert.match(
      forgedStatus.materialValidationBlockers.join("\n"),
      /role-separated.*reuse signer/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-status rejects signed role files with shadow signature metadata", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-signature-shape-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      `0x${"ef".repeat(32)}`,
      "--out",
      requestPath,
    ]);
    const attestations = await writeAttestationsFromRequest(root, requestPath);
    const semantic = JSON.parse(await readFile(attestations.semantic, "utf8"));
    semantic.signature.operatorOverride = "accept external signature out of band";
    semantic.signature.public_key_pem = semantic.signature.publicKeyPem;
    semantic.signature.signed_payload_sha256 =
      semantic.signature.signedPayloadSha256;
    semantic.signature.signatureBase64 = semantic.signature.signature;
    await writeJson(attestations.semantic, semantic);

    const status = await auditBscGroth16AttestationStatus({
      request: requestPath,
      ...trustedSignerOption(),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
    });

    assert.equal(status.readyToFinalize, false);
    const blockers = status.materialValidationBlockers.join("\n");
    assert.match(
      blockers,
      /semantic SCCP circuit attestation signature contains unknown field: operatorOverride/u,
    );
    assert.match(
      blockers,
      /semantic SCCP circuit attestation signature publicKeyPem must not use multiple aliases: publicKeyPem, public_key_pem/u,
    );
    assert.match(
      blockers,
      /semantic SCCP circuit attestation signature signedPayloadSha256 must not use multiple aliases: signedPayloadSha256, signed_payload_sha256/u,
    );
    assert.match(
      blockers,
      /semantic SCCP circuit attestation signature signature must not use multiple aliases: signature, signatureBase64/u,
    );
    assert.doesNotMatch(
      JSON.stringify(status),
      /PRIVATE KEY|privateKey|private_key|mnemonic|seed phrase|password/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("attestation-status rejects untrusted signer fingerprints without leaking keys", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-status-untrusted-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      `0x${"ef".repeat(32)}`,
      "--out",
      requestPath,
    ]);
    const attestations = await writeAttestationsFromRequest(root, requestPath, {
      signingByRole: {
        semantic: {
          privateKey: UNTRUSTED_ATTESTATION_SIGNER.privateKey,
          publicKeyPem: UNTRUSTED_ATTESTATION_PUBLIC_KEY_PEM,
          signerFingerprint: UNTRUSTED_ATTESTATION_SIGNER_FINGERPRINT,
        },
      },
    });

    const status = await main([
      "verify-attestations",
      "--request",
      requestPath,
      "--trusted-attestation-signer",
      TRUSTED_ATTESTATION_SIGNER_FINGERPRINTS.join(","),
      "--semantic-attestation",
      attestations.semantic,
      "--circuit-security-attestation",
      attestations.security,
      "--trusted-setup-attestation",
      attestations.setup,
      "--reproducible-build-attestation",
      attestations.reproducible,
    ]);

    assert.equal(status.readyToFinalize, false);
    assert.match(
      status.materialValidationBlockers.join("\n"),
      /semantic SCCP circuit attestation signature signerFingerprint is not trusted/u,
    );
    assert.doesNotMatch(
      JSON.stringify(status),
      /PRIVATE KEY|privateKey|private_key|mnemonic|seed phrase|password/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("finalize-attestations accepts role files produced by sign-attestation", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-finalize-signed-cli-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      result.reproducibleBuildToolchainSha256,
      "--out",
      requestPath,
    ]);
    const keyFiles = {
      semantic: await writePrivateKeyPem(join(root, "semantic-key.pem"), TEST_ATTESTATION_SIGNER),
      security: await writePrivateKeyPem(join(root, "security-key.pem"), SECURITY_ATTESTATION_SIGNER),
      setup: await writePrivateKeyPem(join(root, "setup-key.pem"), SETUP_ATTESTATION_SIGNER),
      reproducible: await writePrivateKeyPem(join(root, "reproducible-key.pem"), REPRODUCIBLE_ATTESTATION_SIGNER),
    };
    const signed = {
      semantic: join(root, "semantic.json"),
      security: join(root, "security.json"),
      setup: join(root, "setup.json"),
      reproducible: join(root, "reproducible.json"),
    };
    await main([
      "sign-attestation",
      "--request",
      requestPath,
      "--role",
      "semantic",
      "--private-key-pem",
      keyFiles.semantic,
      "--out",
      signed.semantic,
    ]);
    await main([
      "sign-attestation",
      "--request",
      requestPath,
      "--role",
      "circuit-security",
      "--private-key-pem",
      keyFiles.security,
      "--out",
      signed.security,
    ]);
    await main([
      "sign-attestation",
      "--request",
      requestPath,
      "--role",
      "trusted-setup",
      "--private-key-pem",
      keyFiles.setup,
      "--out",
      signed.setup,
    ]);
    await main([
      "sign-attestation",
      "--request",
      requestPath,
      "--role",
      "reproducible-build",
      "--private-key-pem",
      keyFiles.reproducible,
      "--out",
      signed.reproducible,
    ]);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      join(root, "out", "verification_key.json"),
    );

    const finalized = await finalizeBscGroth16Attestations({
      request: requestPath,
      ...trustedSignerOption(),
      "semantic-attestation": signed.semantic,
      "circuit-security-attestation": signed.security,
      "trusted-setup-attestation": signed.setup,
      "reproducible-build-attestation": signed.reproducible,
      "snarkjs-bin": snarkjsStub,
      "out-dir": join(root, "finalized"),
    });

    assert.equal(finalized.productionReady, true);
    const manifest = JSON.parse(await readFile(finalized.manifest, "utf8"));
    assert.equal(
      manifest.attestations.circuitSecurity.signature.signerFingerprint,
      SECURITY_ATTESTATION_SIGNER_FINGERPRINT,
    );
    assert.equal(
      manifest.attestations.trustedSetup.signature.signerFingerprint,
      SETUP_ATTESTATION_SIGNER_FINGERPRINT,
    );
    assert.equal(
      manifest.attestations.reproducibleBuild.signature.signerFingerprint,
      REPRODUCIBLE_ATTESTATION_SIGNER_FINGERPRINT,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("proof-self-test writes a manifest-bound SnarkJS proof report", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-proof-self-test-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const witnessWasm = join(
      candidate.outDir,
      `${BSC_FULL_SCCP_CIRCUIT_PROFILE}_js`,
      `${BSC_FULL_SCCP_CIRCUIT_PROFILE}.wasm`,
    );
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    const out = join(root, "proof-self-test.json");

    const result = await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--witness-wasm",
      witnessWasm,
      "--snarkjs-bin",
      snarkjsStub,
      "--out",
      out,
    ]);
    const report = JSON.parse(await readFile(out, "utf8"));

    assert.equal(result.ok, true);
    assert.equal(result.out, out);
    assert.equal(report.schema, BSC_GROTH16_PROOF_SELF_TEST_SCHEMA);
    assert.equal(report.manifest.sha256, sha256Hex(await readFile(candidate.manifest)));
    assert.equal(report.artifacts.r1cs.sha256, sha256Hex(await readFile(candidate.r1cs)));
    assert.equal(
      report.artifacts.provingKey.sha256,
      sha256Hex(await readFile(candidate.zkey)),
    );
    assert.equal(report.artifacts.witnessWasm.sha256, sha256Hex(await readFile(witnessWasm)));
    assert.deepEqual(report.sample.publicSignalNames, BSC_GROTH16_PUBLIC_SIGNAL_NAMES);
    assert.equal(report.sample.publicSignalWords.length, 9);
    assert.deepEqual(report.publicSignals, report.sample.publicSignalWords);
    assert.equal(report.snarkjs.wtnsCalculate, true);
    assert.equal(report.snarkjs.groth16Prove, true);
    assert.equal(report.snarkjs.groth16Verify, true);
    assert.equal(report.adversarialChecks.publicSignalMismatch.attempted, 9);
    assert.equal(report.adversarialChecks.publicSignalMismatch.rejected, 9);
    assert.equal(report.adversarialChecks.publicSignalMismatch.cases.length, 9);
    assert.deepEqual(
      report.adversarialChecks.publicSignalMismatch.cases.map((entry) => entry.name),
      BSC_GROTH16_PUBLIC_SIGNAL_NAMES,
    );
    assert.equal(report.adversarialChecks.nonBooleanValueBit.attempted, 1);
    assert.equal(report.adversarialChecks.nonBooleanValueBit.rejected, 1);
    assert.equal(
      report.adversarialChecks.nonBooleanValueBit.case.inputName,
      "messageIdBits",
    );
    assert.match(result.proofHash, /^0x[0-9a-f]{64}$/u);
    assert.match(result.publicSignalsHash, /^0x[0-9a-f]{64}$/u);
    assert.match(result.witnessHash, /^0x[0-9a-f]{64}$/u);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("proof-self-test rejects manifests that are not production-ready", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-proof-unready-"));
  try {
    const candidate = await writePreflightCandidate(root, {
      manifest: {
        productionReady: false,
        productionBlockers: [],
      },
    });

    await assert.rejects(
      () =>
        main([
          "proof-self-test",
          "--manifest",
          candidate.manifest,
          "--snarkjs-bin",
          join(root, "must-not-run-snarkjs"),
          "--out",
          join(root, "proof-self-test.json"),
        ]),
      /proof-self-test requires a productionReady Groth16 material manifest/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("proof-self-test can refresh explicit testnet candidate evidence without marking it ready", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-proof-candidate-"));
  try {
    const candidate = await writePreflightCandidate(root, {
      manifest: {
        productionReady: false,
        productionBlockers: ["candidate awaits external ceremony attestations"],
      },
    });
    const witnessWasm = join(
      candidate.outDir,
      `${BSC_FULL_SCCP_CIRCUIT_PROFILE}_js`,
      `${BSC_FULL_SCCP_CIRCUIT_PROFILE}.wasm`,
    );
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    const out = join(candidate.outDir, "testnet-bsc-groth16-proof-self-test.json");

    const result = await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--witness-wasm",
      witnessWasm,
      "--snarkjs-bin",
      snarkjsStub,
      "--allow-unready-candidate",
      "true",
      "--out",
      out,
    ]);
    const report = JSON.parse(await readFile(out, "utf8"));
    const circomStub = await writeCircomStub(root);
    const preflight = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(result.ok, true);
    assert.equal(report.manifest.productionReady, false);
    assert.deepEqual(report.manifest.productionBlockers, [
      "candidate awaits external ceremony attestations",
    ]);
    assert.equal(report.adversarialChecks.publicSignalMismatch.rejected, 9);
    assert.equal(report.adversarialChecks.nonBooleanValueBit.rejected, 1);
    assert.equal(preflight.ready, false);
    assert.match(
      preflight.problems.join("\n"),
      /proof self-test report blocker: proof self-test manifest\.productionReady must be true/u,
    );
    assert.doesNotMatch(
      preflight.problems.join("\n"),
      /adversarialChecks block is required/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("proof-self-test can refresh explicit mainnet candidate evidence without marking it ready", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-proof-mainnet-refresh-"));
  try {
    const candidate = await writePreflightCandidate(root, {
      bscNetwork: "mainnet",
      manifest: {
        productionReady: false,
        productionBlockers: ["mainnet candidate awaits external ceremony attestations"],
      },
    });
    const witnessWasm = join(
      candidate.outDir,
      `${BSC_FULL_SCCP_CIRCUIT_PROFILE}_js`,
      `${BSC_FULL_SCCP_CIRCUIT_PROFILE}.wasm`,
    );
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    const out = join(candidate.outDir, "mainnet-bsc-groth16-proof-self-test.json");

    const result = await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--witness-wasm",
      witnessWasm,
      "--snarkjs-bin",
      snarkjsStub,
      "--allow-unready-mainnet-candidate",
      "true",
      "--out",
      out,
    ]);
    const report = JSON.parse(await readFile(out, "utf8"));
    const circomStub = await writeCircomStub(root);
    const preflight = await preflightBscGroth16Material({
      "bsc-network": "mainnet",
      "out-dir": candidate.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(result.ok, true);
    assert.equal(report.bscNetwork, "mainnet");
    assert.equal(report.chainIdHex, BSC_MAINNET_CHAIN_ID_HEX);
    assert.equal(report.networkIdHex, BSC_MAINNET_NETWORK_ID_HEX);
    assert.equal(report.manifest.productionReady, false);
    assert.deepEqual(report.manifest.productionBlockers, [
      "mainnet candidate awaits external ceremony attestations",
    ]);
    assert.equal(report.adversarialChecks.publicSignalMismatch.rejected, 9);
    assert.equal(report.adversarialChecks.nonBooleanValueBit.rejected, 1);
    assert.equal(preflight.ready, false);
    assert.match(
      preflight.problems.join("\n"),
      /proof self-test report blocker: proof self-test manifest\.productionReady must be true/u,
    );
    assert.doesNotMatch(
      preflight.problems.join("\n"),
      /adversarialChecks block is required/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("proof-self-test refuses unready mainnet candidate reports even with opt-in", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-proof-mainnet-candidate-"));
  try {
    const candidate = await writePreflightCandidate(root, {
      bscNetwork: "mainnet",
      manifest: {
        productionReady: false,
        productionBlockers: ["candidate awaits external ceremony attestations"],
      },
    });
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );

    await assert.rejects(
      () =>
        main([
          "proof-self-test",
          "--manifest",
          candidate.manifest,
          "--snarkjs-bin",
          snarkjsStub,
          "--allow-unready-candidate",
          "true",
          "--out",
          join(root, "proof-self-test.json"),
        ]),
      /--allow-unready-candidate is only allowed for testnet candidate proof reports/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("proof-self-test refuses mainnet candidate flag on testnet reports", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-proof-testnet-mainnet-flag-"));
  try {
    const candidate = await writePreflightCandidate(root, {
      manifest: {
        productionReady: false,
        productionBlockers: ["candidate awaits external ceremony attestations"],
      },
    });
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );

    await assert.rejects(
      () =>
        main([
          "proof-self-test",
          "--manifest",
          candidate.manifest,
          "--snarkjs-bin",
          snarkjsStub,
          "--allow-unready-mainnet-candidate",
          "true",
          "--out",
          join(root, "proof-self-test.json"),
        ]),
      /--allow-unready-mainnet-candidate is only allowed for mainnet candidate proof reports/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("proof-self-test rejects production-ready manifests with unresolved blockers", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-proof-blockers-"));
  try {
    const candidate = await writePreflightCandidate(root, {
      manifest: {
        productionReady: true,
        productionBlockers: ["missing independent semantic audit"],
      },
    });

    await assert.rejects(
      () =>
        main([
          "proof-self-test",
          "--manifest",
          candidate.manifest,
          "--snarkjs-bin",
          join(root, "must-not-run-snarkjs"),
          "--out",
          join(root, "proof-self-test.json"),
        ]),
      /proof-self-test requires a blocker-free Groth16 material manifest: missing independent semantic audit/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("proof-self-test rejects malformed production blocker metadata", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-proof-bad-blockers-"));
  try {
    const candidate = await writePreflightCandidate(root, {
      manifest: {
        productionReady: true,
        productionBlockers: "missing signed setup transcript",
      },
    });

    await assert.rejects(
      () =>
        main([
          "proof-self-test",
          "--manifest",
          candidate.manifest,
          "--snarkjs-bin",
          join(root, "must-not-run-snarkjs"),
          "--out",
          join(root, "proof-self-test.json"),
        ]),
      /proof-self-test requires material manifest productionBlockers to be an empty array/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("proof-self-test rejects fixture-labelled manifest references before SnarkJS", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-proof-fixture-ref-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const fixtureR1cs = join(candidate.outDir, "fixture-full-message.r1cs");
    await writeFile(fixtureR1cs, await readFile(candidate.r1cs));
    const manifest = JSON.parse(await readFile(candidate.manifest, "utf8"));
    manifest.artifacts.r1cs = {
      path: fixtureR1cs,
      sha256: sha256Hex(await readFile(fixtureR1cs)),
    };
    await writeJson(candidate.manifest, manifest);

    await assert.rejects(
      () =>
        main([
          "proof-self-test",
          "--manifest",
          candidate.manifest,
          "--snarkjs-bin",
          join(root, "must-not-run-snarkjs"),
          "--out",
          join(root, "proof-self-test.json"),
        ]),
      /material manifest references are not production-ready: material manifest artifacts\.r1cs\.path must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("proof-self-test rejects forged SnarkJS public signals", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-proof-forged-public-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const witnessWasm = join(
      candidate.outDir,
      `${BSC_FULL_SCCP_CIRCUIT_PROFILE}_js`,
      `${BSC_FULL_SCCP_CIRCUIT_PROFILE}.wasm`,
    );
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      {
        supportProofSelfTest: true,
        publicSignalsOverride: Array.from({ length: 9 }, () => "0"),
      },
    );

    await assert.rejects(
      () =>
        main([
          "proof-self-test",
          "--manifest",
          candidate.manifest,
          "--witness-wasm",
          witnessWasm,
          "--snarkjs-bin",
          snarkjsStub,
          "--out",
          join(root, "proof-self-test.json"),
        ]),
      /SnarkJS proof self-test public signals mismatch: public signal 0/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("proof-self-test rejects witness calculators that accept adversarial assignments", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-proof-adversarial-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const witnessWasm = join(
      candidate.outDir,
      `${BSC_FULL_SCCP_CIRCUIT_PROFILE}_js`,
      `${BSC_FULL_SCCP_CIRCUIT_PROFILE}.wasm`,
    );
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true, acceptInvalidWitnesses: true },
    );

    await assert.rejects(
      () =>
        main([
          "proof-self-test",
          "--manifest",
          candidate.manifest,
          "--witness-wasm",
          witnessWasm,
          "--snarkjs-bin",
          snarkjsStub,
          "--out",
          join(root, "proof-self-test.json"),
        ]),
      /SnarkJS proof self-test adversarial publicSignalMismatch\.message_id was accepted/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("proof-self-test rejects failed SnarkJS verification", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-proof-verify-fail-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const witnessWasm = join(
      candidate.outDir,
      `${BSC_FULL_SCCP_CIRCUIT_PROFILE}_js`,
      `${BSC_FULL_SCCP_CIRCUIT_PROFILE}.wasm`,
    );
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true, failProofVerify: true },
    );

    await assert.rejects(
      () =>
        main([
          "proof-self-test",
          "--manifest",
          candidate.manifest,
          "--witness-wasm",
          witnessWasm,
          "--snarkjs-bin",
          snarkjsStub,
          "--out",
          join(root, "proof-self-test.json"),
        ]),
      /groth16 verify .* failed with exit 3.*forced proof verification failure/us,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("proof-self-test rejects manifest-bound proof artifact hash drift", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-proof-hash-drift-"));
  try {
    const candidate = await writePreflightCandidate(root);
    await writeFile(candidate.zkey, "tampered proving key");
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );

    await assert.rejects(
      () =>
        main([
          "proof-self-test",
          "--manifest",
          candidate.manifest,
          "--snarkjs-bin",
          snarkjsStub,
          "--out",
          join(root, "proof-self-test.json"),
        ]),
      /proving key sha256 must match material manifest/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("finalize-attestations materializes production-ready manifests from signed request packages", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-finalize-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    const requestResult = await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      result.reproducibleBuildToolchainSha256,
      "--out",
      requestPath,
    ]);
    const attestations = await writeAttestationsFromRequest(root, requestPath);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      join(root, "out", "verification_key.json"),
    );
    const finalized = await finalizeBscGroth16Attestations({
      request: requestPath,
      ...trustedSignerOption(),
      "semantic-attestation": attestations.semantic,
      "circuit-security-attestation": attestations.security,
      "trusted-setup-attestation": attestations.setup,
      "reproducible-build-attestation": attestations.reproducible,
      "snarkjs-bin": snarkjsStub,
      "out-dir": join(root, "finalized"),
    });

    assert.equal(requestResult.ok, true);
    assert.equal(finalized.productionReady, true);
    assert.deepEqual(finalized.productionBlockers, []);
    assert.equal(finalized.request, requestPath);
    assert.equal(finalized.requestSha256, sha256Hex(await readFile(requestPath)));
    assert.equal(finalized.requestManifest, result.manifest);
    assert.equal(finalized.requestManifestSha256, requestResult.manifestSha256);
    const manifest = JSON.parse(await readFile(finalized.manifest, "utf8"));
    assert.equal(manifest.productionReady, true);
    assert.equal(
      manifest.attestations.semanticSccpCircuit.signature.signerFingerprint,
      TEST_ATTESTATION_SIGNER_FINGERPRINT,
    );
    assert.deepEqual(
      finalized.requestSignedPayloadSha256,
      requestResult.signedPayloadSha256,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("finalize-attestations refuses signed role bodies that drift from the request package", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-finalize-drift-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      `0x${"ef".repeat(32)}`,
      "--out",
      requestPath,
    ]);
    const attestations = await writeAttestationsFromRequest(root, requestPath, {
      semantic: {
        negativeCaseCoverage: false,
      },
    });
    const snarkjsStub = await writeSnarkjsStub(
      root,
      join(root, "out", "verification_key.json"),
    );

    await assert.rejects(
      () =>
        finalizeBscGroth16Attestations({
          request: requestPath,
          ...trustedSignerOption(),
          "semantic-attestation": attestations.semantic,
          "circuit-security-attestation": attestations.security,
          "trusted-setup-attestation": attestations.setup,
          "reproducible-build-attestation": attestations.reproducible,
          "snarkjs-bin": snarkjsStub,
          "out-dir": join(root, "finalized"),
        }),
      /semantic SCCP circuit signed attestation body must match attestation request package/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("finalize-attestations refuses stale request role payload hashes", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-finalize-stale-role-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      `0x${"ef".repeat(32)}`,
      "--out",
      requestPath,
    ]);
    const request = JSON.parse(await readFile(requestPath, "utf8"));
    request.roles.semanticSccpCircuit.body.negativeCaseCoverage = false;
    await writeJson(requestPath, request);
    const attestations = await writeAttestationsFromRequest(root, requestPath);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      join(root, "out", "verification_key.json"),
    );

    await assert.rejects(
      () =>
        finalizeBscGroth16Attestations({
          request: requestPath,
          ...trustedSignerOption(),
          "semantic-attestation": attestations.semantic,
          "circuit-security-attestation": attestations.security,
          "trusted-setup-attestation": attestations.setup,
          "reproducible-build-attestation": attestations.reproducible,
          "snarkjs-bin": snarkjsStub,
          "out-dir": join(root, "finalized"),
        }),
      /semantic SCCP circuit signedPayloadSha256 must match role body/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("finalize-attestations refuses request packages whose manifest binding drifts", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-finalize-manifest-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      `0x${"ef".repeat(32)}`,
      "--out",
      requestPath,
    ]);
    const request = JSON.parse(await readFile(requestPath, "utf8"));
    request.manifest.sha256 = `0x${"55".repeat(32)}`;
    await writeJson(requestPath, request);
    const attestations = await writeAttestationsFromRequest(root, requestPath);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      join(root, "out", "verification_key.json"),
    );

    await assert.rejects(
      () =>
        finalizeBscGroth16Attestations({
          request: requestPath,
          ...trustedSignerOption(),
          "semantic-attestation": attestations.semantic,
          "circuit-security-attestation": attestations.security,
          "trusted-setup-attestation": attestations.setup,
          "reproducible-build-attestation": attestations.reproducible,
          "snarkjs-bin": snarkjsStub,
          "out-dir": join(root, "finalized"),
        }),
      /manifest\.sha256 must match referenced material manifest/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("finalize-attestations refuses request roles that are not ready for signature", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-finalize-not-ready-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      `0x${"ef".repeat(32)}`,
      "--out",
      requestPath,
    ]);
    const request = JSON.parse(await readFile(requestPath, "utf8"));
    request.roles.trustedSetup.readyForSignature = false;
    request.roles.trustedSetup.blockers = ["operator did not publish ceremony transcript"];
    await writeJson(requestPath, request);
    const attestations = await writeAttestationsFromRequest(root, requestPath);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      join(root, "out", "verification_key.json"),
    );

    await assert.rejects(
      () =>
        finalizeBscGroth16Attestations({
          request: requestPath,
          ...trustedSignerOption(),
          "semantic-attestation": attestations.semantic,
          "circuit-security-attestation": attestations.security,
          "trusted-setup-attestation": attestations.setup,
          "reproducible-build-attestation": attestations.reproducible,
          "snarkjs-bin": snarkjsStub,
          "out-dir": join(root, "finalized"),
        }),
      /trusted setup role is not ready for signature/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("finalize-attestations refuses production blockers after signed request matching", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-finalize-blocker-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(root, "request.json");
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      `0x${"ef".repeat(32)}`,
      "--out",
      requestPath,
    ]);
    const reusedSigner = defaultAttestationSigning().semantic;
    const attestations = await writeAttestationsFromRequest(root, requestPath, {
      signingByRole: {
        semantic: reusedSigner,
        security: reusedSigner,
        setup: reusedSigner,
        reproducible: reusedSigner,
      },
    });
    const snarkjsStub = await writeSnarkjsStub(
      root,
      join(root, "out", "verification_key.json"),
    );

    await assert.rejects(
      () =>
        finalizeBscGroth16Attestations({
          request: requestPath,
          ...trustedSignerOption(),
          "semantic-attestation": attestations.semantic,
          "circuit-security-attestation": attestations.security,
          "trusted-setup-attestation": attestations.setup,
          "reproducible-build-attestation": attestations.reproducible,
          "snarkjs-bin": snarkjsStub,
          "out-dir": join(root, "finalized"),
        }),
      /attestation finalization did not produce productionReady material: .*role-separated.*reuse signer/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight reports missing toolchain and production artifacts", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-missing-"));
  try {
    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": join(root, "missing"),
      "circom-bin": join(root, "missing-circom"),
      "snarkjs-bin": join(root, "missing-snarkjs"),
    });

    assert.equal(result.ready, false);
    assert.equal(result.toolchainReady, false);
    assert.equal(result.artifactReady, false);
    assert.equal(result.toolchain.circom.ok, false);
    assert.equal(result.toolchain.snarkjs.ok, false);
    assert.deepEqual(
      result.missing.sort(),
      [
        "bscVerifierKey",
        "circuitSource",
        "manifest",
        "proofSelfTest",
        "provingKey",
        "r1cs",
        "snarkjsVerificationKey",
        "symbols",
        "witnessWasm",
      ].sort(),
    );
    assert.match(result.problems.join("\n"), /Circom compiler probe failed/u);
    assert.match(result.problems.join("\n"), /SnarkJS probe failed/u);
    assert.match(result.problems.join("\n"), /r1cs artifact is missing/u);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight discovers configured local Groth16 toolchain binaries", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-toolchain-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const sourceCircomStub = await writeCircomStub(root);
    const sourceSnarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    const toolchainRoot = join(root, "toolchain");
    const circomTool = await copyExecutableFixture(
      sourceCircomStub,
      join(toolchainRoot, "cargo", "bin", "circom"),
    );
    const snarkjsTool = await copyExecutableFixture(
      sourceSnarkjsStub,
      join(toolchainRoot, "node_modules", ".bin", "snarkjs"),
    );
    await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--snarkjs-bin",
      snarkjsTool,
      "--out",
      join(candidate.outDir, "testnet-bsc-groth16-proof-self-test.json"),
    ]);

    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "toolchain-root": toolchainRoot,
    });

    assert.equal(result.ready, true);
    assert.equal(result.toolchainReady, true);
    assert.equal(result.toolchainRoot, toolchainRoot);
    assert.equal(result.toolchainRootExplicit, true);
    assert.equal(result.toolchain.circom.command, circomTool);
    assert.equal(result.toolchain.snarkjs.command, snarkjsTool);
    assert.equal(
      result.commands.compile.startsWith(`${circomTool} `),
      true,
    );
    assert.equal(
      result.commands.r1csInfo.startsWith(`${snarkjsTool} r1cs info `),
      true,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight fails closed when a present attestation request has blocked roles", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-blocked-request-"));
  try {
    const { inputs, result } = await writeAttestationRequestCandidate(root);
    const canonical = {
      circuitSource: join(result.outDir, `${BSC_FULL_SCCP_CIRCUIT_PROFILE}.circom`),
      r1cs: join(result.outDir, `${BSC_FULL_SCCP_CIRCUIT_PROFILE}.r1cs`),
      witnessWasm: join(
        result.outDir,
        `${BSC_FULL_SCCP_CIRCUIT_PROFILE}_js`,
        `${BSC_FULL_SCCP_CIRCUIT_PROFILE}.wasm`,
      ),
      symbols: join(result.outDir, `${BSC_FULL_SCCP_CIRCUIT_PROFILE}.sym`),
      provingKey: join(result.outDir, `${BSC_FULL_SCCP_CIRCUIT_PROFILE}.final.zkey`),
      snarkjsVerificationKey: join(
        result.outDir,
        `${BSC_FULL_SCCP_CIRCUIT_PROFILE}.snarkjs-verification-key.json`,
      ),
    };
    await mkdir(dirname(canonical.witnessWasm), { recursive: true });
    await writeFile(canonical.circuitSource, await readFile(inputs.circuitSource));
    await writeFile(canonical.r1cs, await readFile(inputs.r1cs));
    await writeFile(canonical.witnessWasm, await readFile(inputs.witnessWasm));
    await writeFile(canonical.symbols, "1,1,0,main.publicSignals[0]\n");
    await writeFile(canonical.provingKey, await readFile(inputs.zkey));
    await writeFile(
      canonical.snarkjsVerificationKey,
      await readFile(inputs.verificationKeyPath),
    );

    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    manifest.artifacts.circuitSource.path = canonical.circuitSource;
    manifest.artifacts.circuitSource.sha256 = sha256Hex(
      await readFile(canonical.circuitSource),
    );
    manifest.artifacts.r1cs.path = canonical.r1cs;
    manifest.artifacts.r1cs.sha256 = sha256Hex(await readFile(canonical.r1cs));
    manifest.artifacts.provingKey.path = canonical.provingKey;
    manifest.artifacts.provingKey.sha256 = sha256Hex(
      await readFile(canonical.provingKey),
    );
    manifest.artifacts.snarkjsVerificationKey.path =
      canonical.snarkjsVerificationKey;
    manifest.artifacts.snarkjsVerificationKey.sha256 = sha256Hex(
      await readFile(canonical.snarkjsVerificationKey),
    );
    if (manifest.artifacts.witnessWasm) {
      manifest.artifacts.witnessWasm.path = canonical.witnessWasm;
      manifest.artifacts.witnessWasm.sha256 = sha256Hex(
        await readFile(canonical.witnessWasm),
      );
    }
    manifest.productionReady = true;
    manifest.productionBlockers = [];
    await writeJson(result.manifest, manifest);

    const requestPath = join(result.outDir, "testnet-bsc-groth16-attestation-request.json");
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--toolchain-sha256",
      result.reproducibleBuildToolchainSha256,
      "--out",
      requestPath,
    ]);
    const request = JSON.parse(await readFile(requestPath, "utf8"));
    request.roles.trustedSetup.readyForSignature = false;
    request.roles.trustedSetup.blockers = [
      "ceremony transcript has not passed independent production review",
    ];
    await writeJson(requestPath, request);

    const snarkjsStub = await writeSnarkjsStub(root, inputs.verificationKeyPath, {
      supportProofSelfTest: true,
    });
    await main([
      "proof-self-test",
      "--manifest",
      result.manifest,
      "--witness-wasm",
      canonical.witnessWasm,
      "--snarkjs-bin",
      snarkjsStub,
      "--out",
      join(result.outDir, "testnet-bsc-groth16-proof-self-test.json"),
    ]);

    const preflight = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": result.outDir,
      "circom-bin": await writeCircomStub(root),
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(preflight.ready, false);
    assert.equal(preflight.artifactReady, false);
    assert.match(
      preflight.problems.join("\n"),
      /attestation request roles are not ready for signature: trustedSetup/u,
    );
    assert.match(
      preflight.problems.join("\n"),
      /ceremony transcript has not passed independent production review/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight explicit Groth16 commands override configured toolchain root", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-toolchain-override-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--snarkjs-bin",
      snarkjsStub,
      "--out",
      join(candidate.outDir, "testnet-bsc-groth16-proof-self-test.json"),
    ]);

    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "toolchain-root": join(root, "empty-toolchain"),
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(result.ready, true);
    assert.equal(result.toolchainReady, true);
    assert.equal(result.toolchain.circom.command, circomStub);
    assert.equal(result.toolchain.snarkjs.command, snarkjsStub);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight empty explicit Groth16 toolchain root fails closed", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-toolchain-empty-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const toolchainRoot = join(root, "empty-toolchain");
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--snarkjs-bin",
      snarkjsStub,
      "--out",
      join(candidate.outDir, "testnet-bsc-groth16-proof-self-test.json"),
    ]);

    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "toolchain-root": toolchainRoot,
    });

    assert.equal(result.ready, false);
    assert.equal(result.toolchainReady, false);
    assert.equal(result.artifactReady, true);
    assert.equal(
      result.toolchain.circom.command,
      join(toolchainRoot, "cargo", "bin", "circom"),
    );
    assert.equal(
      result.toolchain.snarkjs.command,
      join(toolchainRoot, "node_modules", ".bin", "snarkjs"),
    );
    assert.match(result.problems.join("\n"), /Circom compiler probe failed/u);
    assert.match(result.problems.join("\n"), /SnarkJS probe failed/u);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight marks a complete full-message production bundle ready", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-ready-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--snarkjs-bin",
      snarkjsStub,
      "--out",
      join(candidate.outDir, "testnet-bsc-groth16-proof-self-test.json"),
    ]);

    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(result.ready, true);
    assert.equal(result.toolchainReady, true);
    assert.equal(result.artifactReady, true);
    assert.deepEqual(result.missing, []);
    assert.deepEqual(result.problems, []);
    assert.equal(result.artifacts.proofSelfTest.verified, true);
    assert.equal(result.artifacts.circuitSource.checks.fullMessageCircuit, true);
    assert.equal(result.artifacts.circuitSource.checks.labelBindingCount, 9);
    assert.equal(result.artifacts.r1cs.r1csHeader.nPubInputs, 9);
    assert.equal(result.artifacts.r1cs.r1csHeader.nConstraints, 2_154_897);
    assert.equal(
      result.artifacts.bscVerifierKey.verifierKeyHash,
      candidate.verifierKeyHash,
    );
    assert.match(result.commands.materialize, /--trusted-setup-transcript/u);
    assert.match(result.commands.materialize, /--reproducible-build-transcript/u);
    assert.match(result.commands.materialize, /--witness-wasm/u);
    assert.match(result.commands.materialize, /--snarkjs-bin/u);
    assert.doesNotMatch(
      result.commands.materialize,
      /--semantic-attestation|--trusted-attestation-signer/u,
    );
    assert.match(result.commands.toolchainFingerprint, /toolchain-fingerprint/u);
    assert.match(result.commands.toolchainFingerprint, /--circom-bin/u);
    assert.match(result.commands.toolchainFingerprint, /--snarkjs-bin/u);
    assert.match(result.commands.toolchainFingerprint, /--transcript/u);
    assert.match(result.commands.proofSelfTest, /proof-self-test --manifest/u);
    assert.match(result.commands.proofSelfTest, /--witness-wasm/u);
    assert.match(result.commands.proofSelfTest, /--snarkjs-bin/u);
    assert.match(result.commands.attestationRequest, /attestation-request --manifest/u);
    assert.doesNotMatch(result.commands.attestationRequest, /--toolchain-sha256/u);
    assert.match(result.commands.attestationRequest, /--out .*attestation-request\.json/u);
    assert.match(
      result.commands.signAttestation,
      /sign-attestation --request .*attestation-request\.json/u,
    );
    assert.match(
      result.commands.signAttestation,
      /--role semanticSccpCircuit --private-key-pem <ed25519-private-key\.pem>/u,
    );
    assert.match(
      result.commands.attestationStatus,
      /attestation-status --request .*attestation-request\.json/u,
    );
    assert.match(
      result.commands.attestationStatus,
      /--semantic-attestation <semantic-sccp-circuit-attestation\.json>/u,
    );
    assert.match(
      result.commands.finalizeAttestations,
      /finalize-attestations --request .*attestation-request\.json/u,
    );
    assert.match(
      result.commands.finalizeAttestations,
      /--trusted-attestation-signer <0x\.\.\.>/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight summarizes present attestation request readiness", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-request-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(
      result.outDir,
      "testnet-bsc-groth16-attestation-request.json",
    );
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--out",
      requestPath,
    ]);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      join(result.outDir, "verification_key.json"),
    );

    const preflight = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": result.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(preflight.artifacts.attestationRequest.present, true);
    assert.equal(
      preflight.artifacts.attestationRequest.status.readyToFinalize,
      false,
    );
    assert.equal(
      preflight.artifacts.attestationRequest.status.requestValid,
      true,
    );
    assert.deepEqual(
      preflight.artifacts.attestationRequest.status.requestReadyForSignature,
      {
        semanticSccpCircuit: true,
        circuitSecurity: true,
        trustedSetup: true,
        reproducibleBuild: true,
      },
    );
    assert.deepEqual(
      preflight.artifacts.attestationRequest.status.missingSignedRoles.sort(),
      [
        "semanticSccpCircuit",
        "circuitSecurity",
        "trustedSetup",
        "reproducibleBuild",
      ].sort(),
    );
    assert.match(
      preflight.artifacts.attestationRequest.status.nextActions.join("\n"),
      /Sign the missing ready role payloads/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight reports attestation requests with fixture-labelled manifest references", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-fixture-ref-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(
      result.outDir,
      "testnet-bsc-groth16-attestation-request.json",
    );
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--out",
      requestPath,
    ]);
    const manifest = JSON.parse(await readFile(result.manifest, "utf8"));
    const fixtureR1cs = join(result.outDir, "fixture-full-message.r1cs");
    await writeFile(fixtureR1cs, await readFile(manifest.artifacts.r1cs.path));
    manifest.artifacts.r1cs = {
      path: fixtureR1cs,
      sha256: sha256Hex(await readFile(fixtureR1cs)),
    };
    await writeJson(result.manifest, manifest);
    const request = JSON.parse(await readFile(requestPath, "utf8"));
    request.manifest.sha256 = sha256Hex(await readFile(result.manifest));
    request.artifacts.r1cs = manifest.artifacts.r1cs;
    await writeJson(requestPath, request);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      join(result.outDir, "verification_key.json"),
    );

    const preflight = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": result.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    const status = preflight.artifacts.attestationRequest.status;
    assert.equal(status.readyToFinalize, false);
    assert.equal(status.requestValid, false);
    assert.match(
      status.firstProblems.join("\n"),
      /material manifest references are not production-ready: material manifest artifacts\.r1cs\.path must not reference diagnostic, fixture, mock, placeholder, sample, stub, or test-only material/u,
    );
    assert.match(
      preflight.problems.join("\n"),
      /attestation request self-check failed: material manifest references are not production-ready/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight reports stale attestation request packages", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-request-stale-"));
  try {
    const { result } = await writeAttestationRequestCandidate(root);
    const requestPath = join(
      result.outDir,
      "testnet-bsc-groth16-attestation-request.json",
    );
    await main([
      "attestation-request",
      "--manifest",
      result.manifest,
      ...result.attestationRequestEvidenceArgs,
      "--out",
      requestPath,
    ]);
    const request = JSON.parse(await readFile(requestPath, "utf8"));
    request.manifest.sha256 = `0x${"55".repeat(32)}`;
    await writeJson(requestPath, request);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      join(result.outDir, "verification_key.json"),
    );

    const preflight = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": result.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(preflight.ready, false);
    assert.match(
      preflight.problems.join("\n"),
      /attestation request self-check failed: attestation request package manifest\.sha256 must match referenced material manifest/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight rejects tampered proof self-test reports", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-proof-tamper-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    const proofSelfTestPath = join(
      candidate.outDir,
      "testnet-bsc-groth16-proof-self-test.json",
    );
    await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--snarkjs-bin",
      snarkjsStub,
      "--out",
      proofSelfTestPath,
    ]);
    const report = JSON.parse(await readFile(proofSelfTestPath, "utf8"));
    report.publicSignals[0] = "0";
    await writeJson(proofSelfTestPath, report);

    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(result.ready, false);
    assert.equal(result.toolchainReady, true);
    assert.equal(result.artifactReady, false);
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test public signals mismatch: public signal 0/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight rejects proof self-test forged proof bodies", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-proof-forged-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    const proofSelfTestPath = join(
      candidate.outDir,
      "testnet-bsc-groth16-proof-self-test.json",
    );
    await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--snarkjs-bin",
      snarkjsStub,
      "--out",
      proofSelfTestPath,
    ]);
    const report = JSON.parse(await readFile(proofSelfTestPath, "utf8"));
    report.proof.pi_a[0] = "2";
    report.proofHash = sha256Hex(
      Buffer.from(canonicalJson(report.proof), "utf8"),
    );
    await writeJson(proofSelfTestPath, report);

    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(result.ready, false);
    assert.equal(result.toolchainReady, true);
    assert.equal(result.artifactReady, false);
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test embedded Groth16 proof must verify against SnarkJS verification key/u,
    );
    assert.match(
      result.problems.join("\n"),
      /forced proof object verification failure/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight rejects proof self-test artifact hash drift", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-proof-artifact-drift-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    const proofSelfTestPath = join(
      candidate.outDir,
      "testnet-bsc-groth16-proof-self-test.json",
    );
    await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--snarkjs-bin",
      snarkjsStub,
      "--out",
      proofSelfTestPath,
    ]);
    const report = JSON.parse(await readFile(proofSelfTestPath, "utf8"));
    report.artifacts.circuitSource.sha256 = `0x${"44".repeat(32)}`;
    report.artifacts.witnessWasm.sha256 = `0x${"55".repeat(32)}`;
    await writeJson(proofSelfTestPath, report);

    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(result.ready, false);
    assert.equal(result.toolchainReady, true);
    assert.equal(result.artifactReady, false);
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test circuit source sha256 must match/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test witness WASM sha256 must match/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight rejects proof self-test path metadata drift", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-proof-path-drift-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    const proofSelfTestPath = join(
      candidate.outDir,
      "testnet-bsc-groth16-proof-self-test.json",
    );
    await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--snarkjs-bin",
      snarkjsStub,
      "--out",
      proofSelfTestPath,
    ]);
    const report = JSON.parse(await readFile(proofSelfTestPath, "utf8"));
    report.manifest.path = "operator/other-manifest.json";
    report.artifacts.r1cs.path = "operator/other.r1cs";
    report.artifacts.witnessWasm.path = "operator/other.wasm";
    await writeJson(proofSelfTestPath, report);

    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(result.ready, false);
    assert.equal(result.toolchainReady, true);
    assert.equal(result.artifactReady, false);
    const problems = result.problems.join("\n");
    assert.match(
      problems,
      /proof self-test report blocker: proof self-test manifest\.path must be/u,
    );
    assert.match(
      problems,
      /proof self-test report blocker: proof self-test R1CS path must be/u,
    );
    assert.match(
      problems,
      /proof self-test report blocker: proof self-test witness WASM path must be/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight rejects proof self-test reports with unknown shadow fields", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-proof-shadow-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    const proofSelfTestPath = join(
      candidate.outDir,
      "testnet-bsc-groth16-proof-self-test.json",
    );
    await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--snarkjs-bin",
      snarkjsStub,
      "--out",
      proofSelfTestPath,
    ]);
    const report = JSON.parse(await readFile(proofSelfTestPath, "utf8"));
    report.operatorDecision = "accept";
    report.manifest.shadowReady = true;
    report.artifacts.r1cs.shadowHash = report.artifacts.r1cs.sha256;
    report.sample.syntheticInputWords.shadow_signal = "1";
    report.snarkjs.remoteProver = true;
    report.adversarialChecks.publicSignalMismatch.cases[0].acceptedByFallback =
      false;
    report.proof.transcriptHint = "operator-shadow-proof";
    report.proofHash = sha256Hex(
      Buffer.from(canonicalJson(report.proof), "utf8"),
    );
    await writeJson(proofSelfTestPath, report);

    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(result.ready, false);
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test report contains unknown field: operatorDecision/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test manifest contains unknown field: shadowReady/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test R1CS artifact contains unknown field: shadowHash/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test sample\.syntheticInputWords contains unknown field: shadow_signal/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test snarkjs contains unknown field: remoteProver/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test adversarialChecks\.publicSignalMismatch\.cases\[0\] contains unknown field: acceptedByFallback/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test proof contains unknown field: transcriptHint/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight rejects proof self-test profile and public-signal hash drift", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-proof-profile-drift-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    const proofSelfTestPath = join(
      candidate.outDir,
      "testnet-bsc-groth16-proof-self-test.json",
    );
    await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--snarkjs-bin",
      snarkjsStub,
      "--out",
      proofSelfTestPath,
    ]);
    const report = JSON.parse(await readFile(proofSelfTestPath, "utf8"));
    report.chainIdHex = BSC_MAINNET_CHAIN_ID_HEX;
    report.networkIdHex = BSC_MAINNET_NETWORK_ID_HEX;
    report.proofBackend = "diagnostic-groth16-backend";
    report.proofFamily = "fixture-proof-family";
    report.publicSignalsHash = `0x${"66".repeat(32)}`;
    report.proofHash = `0x${"77".repeat(32)}`;
    await writeJson(proofSelfTestPath, report);

    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(result.ready, false);
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test chainIdHex must be 0x61/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test networkIdHex must be 0x0000000000000000000000000000000000000000000000000000000000000061/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test proofBackend must be evm-groth16-bn254-v1/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test proofFamily must be stark-fri-v1/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test publicSignalsHash must match publicSignals/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test proofHash must match proof/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight rejects proof self-test deterministic sample drift", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-proof-sample-drift-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    const proofSelfTestPath = join(
      candidate.outDir,
      "testnet-bsc-groth16-proof-self-test.json",
    );
    await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--snarkjs-bin",
      snarkjsStub,
      "--out",
      proofSelfTestPath,
    ]);
    const report = JSON.parse(await readFile(proofSelfTestPath, "utf8"));
    report.sample.id = "operator-supplied-sample";
    report.sample.syntheticInputWords[BSC_GROTH16_PUBLIC_SIGNAL_NAMES[0]] =
      `0x${"88".repeat(32)}`;
    report.sample.publicSignalWords[0] = "1";
    report.sample.inputSha256 = `0x${"99".repeat(32)}`;
    await writeJson(proofSelfTestPath, report);

    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(result.ready, false);
    const problems = result.problems.join("\n");
    assert.match(
      problems,
      /proof self-test report blocker: proof self-test sample\.id must be/u,
    );
    assert.match(
      problems,
      /proof self-test report blocker: proof self-test sample\.syntheticInputWords must match deterministic BSC Groth16 self-test input/u,
    );
    assert.match(
      problems,
      /proof self-test report blocker: proof self-test sample\.inputSha256 must match deterministic self-test input/u,
    );
    assert.match(
      problems,
      /proof self-test report blocker: proof self-test sample\.publicSignalWords mismatch/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight rejects malformed proof self-test proof coordinates", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-proof-shape-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    const proofSelfTestPath = join(
      candidate.outDir,
      "testnet-bsc-groth16-proof-self-test.json",
    );
    await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--snarkjs-bin",
      snarkjsStub,
      "--out",
      proofSelfTestPath,
    ]);
    const report = JSON.parse(await readFile(proofSelfTestPath, "utf8"));
    report.publicSignals = ["01", ...report.publicSignals.slice(1)];
    report.publicSignalsHash = sha256Hex(
      Buffer.from(canonicalJson(report.publicSignals), "utf8"),
    );
    report.proof = {
      pi_a: ["1", "2"],
      pi_b: [["01", "2"], ["3"], ["4", "5"]],
      pi_c: ["1", "2", "01"],
      protocol: "plonk",
      curve: "bn254",
    };
    report.proofHash = sha256Hex(
      Buffer.from(canonicalJson(report.proof), "utf8"),
    );
    await writeJson(proofSelfTestPath, report);

    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(result.ready, false);
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: SnarkJS public signal 0 must be a canonical decimal BN254 field word/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test proof\.protocol must be groth16/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test proof\.curve must be bn128/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test proof\.pi_a must contain 3 canonical decimal BN254 field words/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test proof\.pi_b\[0\]\[0\] must be a canonical decimal BN254 field word/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test proof\.pi_b\[1\] must contain 2 canonical decimal BN254 field words/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test proof\.pi_c\[2\] must be a canonical decimal BN254 field word/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight rejects proof self-test reports from unready manifests", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-proof-unready-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    const proofSelfTestPath = join(
      candidate.outDir,
      "testnet-bsc-groth16-proof-self-test.json",
    );
    await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--snarkjs-bin",
      snarkjsStub,
      "--out",
      proofSelfTestPath,
    ]);
    const report = JSON.parse(await readFile(proofSelfTestPath, "utf8"));
    report.manifest.productionReady = false;
    report.manifest.productionBlockers = ["stale report generated before audit"];
    await writeJson(proofSelfTestPath, report);

    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(result.ready, false);
    assert.equal(result.toolchainReady, true);
    assert.equal(result.artifactReady, false);
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test manifest\.productionReady must be true/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test manifest\.productionBlockers must be empty: stale report generated before audit/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight rejects proof self-test reports without complete adversarial evidence", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-proof-adversarial-"));
  try {
    const candidate = await writePreflightCandidate(root);
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
      { supportProofSelfTest: true },
    );
    const proofSelfTestPath = join(
      candidate.outDir,
      "testnet-bsc-groth16-proof-self-test.json",
    );
    await main([
      "proof-self-test",
      "--manifest",
      candidate.manifest,
      "--snarkjs-bin",
      snarkjsStub,
      "--out",
      proofSelfTestPath,
    ]);
    const report = JSON.parse(await readFile(proofSelfTestPath, "utf8"));
    report.adversarialChecks.publicSignalMismatch.rejected = 8;
    report.adversarialChecks.publicSignalMismatch.cases.pop();
    delete report.adversarialChecks.nonBooleanValueBit;
    await writeJson(proofSelfTestPath, report);

    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(result.ready, false);
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test adversarial publicSignalMismatch\.rejected must be 9/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test adversarial publicSignalMismatch\.cases must contain 9 entries/u,
    );
    assert.match(
      result.problems.join("\n"),
      /proof self-test report blocker: proof self-test adversarialChecks\.nonBooleanValueBit is required/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("preflight rejects malformed verifier material and non-ready manifests", async () => {
  const root = await mkdtemp(join(tmpdir(), "iroha-bsc-groth16-preflight-bad-"));
  try {
    const candidate = await writePreflightCandidate(root, {
      verifierMaterial: {
        publicInputCount: 8,
        verifierKeyHash: `0x${"aa".repeat(32)}`,
      },
      manifest: {
        productionReady: false,
        productionBlockers: ["missing trusted setup ceremony attestation"],
      },
    });
    const circomStub = await writeCircomStub(root);
    const snarkjsStub = await writeSnarkjsStub(
      root,
      candidate.snarkjsVerificationKey,
    );

    const result = await preflightBscGroth16Material({
      "bsc-network": "testnet",
      "out-dir": candidate.outDir,
      "circom-bin": circomStub,
      "snarkjs-bin": snarkjsStub,
    });

    assert.equal(result.ready, false);
    assert.equal(result.toolchainReady, true);
    assert.equal(result.artifactReady, false);
    assert.match(
      result.problems.join("\n"),
      /BSC verifier key self-check failed/u,
    );
    assert.match(
      result.problems.join("\n"),
      /publicInputCount must be 9/u,
    );
    assert.match(
      result.problems.join("\n"),
      /material manifest is not productionReady/u,
    );
    assert.match(
      result.problems.join("\n"),
      /missing trusted setup ceremony attestation/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("generate command help is exposed through the material CLI", async () => {
  const result = await main(["help"]);

  assert.match(result.help, /sccp_bsc_groth16_material\.mjs generate/u);
  assert.match(
    result.help,
    /sccp_bsc_groth16_material\.mjs toolchain-fingerprint/u,
  );
  assert.match(
    result.help,
    /sccp_bsc_groth16_material\.mjs attestation-request/u,
  );
  assert.match(result.help, /sccp_bsc_groth16_material\.mjs handoff-bundle/u);
  assert.match(result.help, /sccp_bsc_groth16_material\.mjs verify-handoff/u);
  assert.match(
    result.help,
    /sccp_bsc_groth16_material\.mjs sign-attestation/u,
  );
  assert.match(result.help, /sccp_bsc_groth16_material\.mjs proof-self-test/u);
  assert.match(
    result.help,
    /sccp_bsc_groth16_material\.mjs finalize-attestations/u,
  );
  assert.match(
    result.help,
    /materialize .*--trusted-setup-transcript <json> --reproducible-build-transcript <json>/u,
  );
  assert.doesNotMatch(
    result.help,
    /materialize .*--semantic-attestation .*--trusted-attestation-signer/u,
  );
  assert.match(result.help, /sccp_bsc_groth16_material\.mjs preflight/u);
  assert.match(result.help, new RegExp(BSC_SIGNAL_BINDING_CIRCUIT_PROFILE, "u"));
  assert.match(
    result.help,
    new RegExp(DEFAULT_BSC_FULL_MESSAGE_CIRCUIT_SOURCE, "u"),
  );
});

test("material CLI subcommand help returns usage without operator probes", async () => {
  const result = await main(["preflight", "--help"]);

  assert.match(result.help, /sccp_bsc_groth16_material\.mjs preflight/u);
  assert.equal(Object.prototype.hasOwnProperty.call(result, "toolchain"), false);
  assert.equal(Object.prototype.hasOwnProperty.call(result, "artifacts"), false);
});

test("materialize CLI refuses signed attestation inputs and requires finalization flow", async () => {
  await assert.rejects(
    () =>
      main([
        "materialize",
        "--semantic-attestation",
        "semantic-sccp-circuit-attestation.json",
      ]),
    /materialize no longer accepts signed attestation inputs.*--semantic-attestation.*attestation-request.*finalize-attestations/u,
  );

  await assert.rejects(
    () =>
      main([
        "materialize",
        "--trusted-attestation-signer",
        `0x${"12".repeat(32)}`,
      ]),
    /materialize no longer accepts signed attestation inputs.*--trusted-attestation-signer.*attestation-request.*finalize-attestations/u,
  );
});
