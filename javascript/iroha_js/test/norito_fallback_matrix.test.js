"use strict";

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { test as baseTest } from "node:test";
import { fileURLToPath } from "node:url";

import {
  AccountAddress,
  configureCurveSupport,
  curveIdFromAlgorithm,
} from "../src/address.js";
import {
  buildActivateContractInstanceInstruction,
  buildCastPlainBallotInstruction,
  buildCastZkBallotInstruction,
  buildClaimTwitterFollowRewardInstruction,
  buildCreateKaigiInstruction,
  buildFinalizeElectionInstruction,
  buildPersistCouncilForEpochInstruction,
  buildProposeDeployContractInstruction,
  buildRegisterAccountInstruction,
  buildRegisterDomainInstruction,
  buildRegisterKaigiRelayInstruction,
  buildRegisterMultisigInstruction,
  buildRegisterRwaInstruction,
  buildRegisterSmartContractCodeInstruction,
  buildRegisterZkAssetInstruction,
  buildRecordKaigiUsageInstruction,
  buildSetRwaControlsInstruction,
  buildSetRwaKeyValueInstruction,
  buildShieldInstruction,
  buildTransferAssetDefinitionInstruction,
  buildTransferAssetInstruction,
  buildTransferDomainInstruction,
  buildTransferNftInstruction,
  buildUnshieldInstruction,
  buildZkTransferInstruction,
} from "../src/instructionBuilders.js";
import { MultisigSpecBuilder } from "../src/multisig.js";
import {
  makeNativeTest,
  noritoRequiredMethods,
} from "./helpers/native.js";
import { getNativeBinding } from "../src/native.js";

const test = makeNativeTest(baseTest, { require: noritoRequiredMethods });
const nativeBinding = getNativeBinding();
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const ML_DSA_FIXTURE_PATH = path.resolve(
  __dirname,
  "../../../fixtures/account/ml_dsa_public_key.hex",
);
const GOST256_FIXTURE_PATH = path.resolve(
  __dirname,
  "../../../fixtures/account/gost256a_public_key.hex",
);

function hexToBytes(hex) {
  return Buffer.from(hex.replace(/^0x/i, ""), "hex");
}

const ACCOUNT_ID = AccountAddress.fromAccount({
  publicKey: hexToBytes(
    "CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
  ),
}).toI105();
const SAMPLE_ACCOUNT_ID = AccountAddress.fromAccount({
  publicKey: hexToBytes(
    "641297079357229F295938A4B5A333DE35069BF47B9D0704E45805713D13C201",
  ),
}).toI105();
const ASSET_DEFINITION_ID = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
const ASSET_ID = `${ASSET_DEFINITION_ID}#${ACCOUNT_ID}`;
const NFT_ID = "dragon$wonderland";
const RWA_ID =
  "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities";
const MULTISIG_CANONICAL_HEX =
  "0x0a010100030003010001002068f4b6017d0f876a55c80a82b8388a54aad264d367269e2de8be079c935b5f9601000100207ea0e3bd52e207c9d3b0eba65c0704e66fca2d8e165a175218b174fc4160e4130100020020884b8857f4eaa1613c61504db34d4beaf346517a0e31de3cddd4d9b4201d9d0b";
const ML_DSA_PUBLIC_KEY = Buffer.from(
  fs.readFileSync(ML_DSA_FIXTURE_PATH, "utf8").trim(),
  "hex",
);
const GOST256A_PUBLIC_KEY = Buffer.from(
  fs.readFileSync(GOST256_FIXTURE_PATH, "utf8").trim(),
  "hex",
);

function withDisabledNative(fn) {
  const previous = process.env.IROHA_JS_DISABLE_NATIVE;
  process.env.IROHA_JS_DISABLE_NATIVE = "1";
  return Promise.resolve()
    .then(() => fn())
    .finally(() => {
      if (previous === undefined) {
        delete process.env.IROHA_JS_DISABLE_NATIVE;
      } else {
        process.env.IROHA_JS_DISABLE_NATIVE = previous;
      }
    });
}

function decodeInFreshJsOnlyProcess(bytes) {
  const script = `
    import { noritoDecodeInstruction, noritoEncodeInstruction } from './javascript/iroha_js/src/norito.js?fresh-js-only';
    const bytes = Buffer.from(process.argv[1], 'base64');
    const decoded = noritoDecodeInstruction(bytes);
    const reencoded = noritoEncodeInstruction(decoded);
    process.stdout.write(JSON.stringify({
      decoded,
      reencodedHex: reencoded.toString('hex'),
    }));
  `;
  const result = spawnSync(
    process.execPath,
    ["--input-type=module", "-e", script, bytes.toString("base64")],
    {
      cwd: path.resolve(__dirname, "../../.."),
      env: {
        ...process.env,
        IROHA_JS_DISABLE_NATIVE: "1",
      },
      encoding: "utf8",
    },
  );
  if (result.status !== 0) {
    throw new Error(result.stderr || result.stdout || "fresh JS-only decode subprocess failed");
  }
  return JSON.parse(result.stdout);
}

function sampleSpec() {
  return new MultisigSpecBuilder()
    .setQuorum(1)
    .setTransactionTtlMs(60_000)
    .addSignatory(ACCOUNT_ID, 1)
    .build();
}

function destinationAccountIdForCurve(algorithm, publicKey) {
  if (publicKey.length > 0xff) {
    return new AccountAddress(
      { version: 0, classId: 1, normVersion: 1, extFlag: false },
      {
        tag: 1,
        version: 1,
        threshold: 1,
        members: [{ curve: curveIdFromAlgorithm(algorithm), weight: 1, publicKey }],
      },
    ).toI105();
  }
  return AccountAddress.fromAccount({ publicKey, algorithm }).toI105();
}

function nativeEncodeInstruction(instruction) {
  if (!nativeBinding) {
    throw new Error("native binding unavailable");
  }
  return nativeBinding.noritoEncodeInstruction(JSON.stringify(instruction));
}

function nativeDecodeInstruction(bytes) {
  if (!nativeBinding) {
    throw new Error("native binding unavailable");
  }
  return JSON.parse(nativeBinding.noritoDecodeInstruction(bytes));
}

function representativeInstructions() {
  return [
    ["Register.Domain", buildRegisterDomainInstruction({ domainId: "wonderland", metadata: { topic: "tea" } })],
    ["Register.Account", buildRegisterAccountInstruction({ accountId: ACCOUNT_ID, domainId: "wonderland" })],
    ["Transfer.Domain", buildTransferDomainInstruction({ sourceAccountId: ACCOUNT_ID, domainId: "wonderland", destinationAccountId: SAMPLE_ACCOUNT_ID })],
    ["Transfer.AssetDefinition", buildTransferAssetDefinitionInstruction({ sourceAccountId: ACCOUNT_ID, assetDefinitionId: ASSET_DEFINITION_ID, destinationAccountId: SAMPLE_ACCOUNT_ID })],
    ["Transfer.Asset", buildTransferAssetInstruction({ sourceAssetHoldingId: ASSET_ID, quantity: "17", destinationAccountId: SAMPLE_ACCOUNT_ID })],
    ["Transfer.Nft", buildTransferNftInstruction({ sourceAccountId: ACCOUNT_ID, nftId: NFT_ID, destinationAccountId: SAMPLE_ACCOUNT_ID })],
    ["Custom", buildRegisterMultisigInstruction({ accountId: ACCOUNT_ID, spec: sampleSpec() })],
    ["Kaigi.CreateKaigi", buildCreateKaigiInstruction({ id: "wonderland:weekly-sync", host: ACCOUNT_ID, gasRatePerMinute: 120 })],
    ["Kaigi.RecordKaigiUsage", buildRecordKaigiUsageInstruction({ callId: "wonderland:weekly-sync", durationMs: 60_000, billedGas: 512 })],
    ["Kaigi.RegisterKaigiRelay", buildRegisterKaigiRelayInstruction({ relayId: ACCOUNT_ID, hpkePublicKey: Buffer.alloc(32, 0xaa), bandwidthClass: 7 })],
    ["Governance.ProposeDeployContract", buildProposeDeployContractInstruction({ namespace: "apps", contractId: "ledger", codeHash: Buffer.alloc(32, 0xaa), abiHash: Buffer.alloc(32, 0xbb), abiVersion: "1" })],
    ["Governance.CastZkBallot", buildCastZkBallotInstruction({ electionId: "ref-1", proof: Buffer.from([0x01, 0x02]), publicInputs: { tally: "aye" } })],
    ["Governance.CastPlainBallot", buildCastPlainBallotInstruction({ referendumId: "ref-2", owner: ACCOUNT_ID, amount: "1000", durationBlocks: 50, direction: "nay" })],
    ["Governance.PersistCouncilForEpoch", buildPersistCouncilForEpochInstruction({ epoch: 10, members: [ACCOUNT_ID], candidatesCount: 5, derivedBy: "fallback" })],
    ["Social.ClaimTwitterFollowReward", buildClaimTwitterFollowRewardInstruction({ bindingHash: { pepper_id: "twitter-follow", digest: Buffer.alloc(32, 0xaa) } })],
    ["Smart.RegisterSmartContractCode", buildRegisterSmartContractCodeInstruction({ manifest: { codeHash: Buffer.alloc(32, 0xaa), abiHash: Buffer.alloc(32, 0xbb), compilerFingerprint: "rustc-1.79", featuresBitmap: 42, accessSetHints: { readKeys: ["account:alice"], writeKeys: ["contract:foo"] } } })],
    ["Smart.ActivateContractInstance", buildActivateContractInstanceInstruction({ namespace: "apps", contractId: "governance", codeHash: Buffer.alloc(32, 0x44) })],
    ["zk.RegisterZkAsset", buildRegisterZkAssetInstruction({ assetDefinitionId: ASSET_DEFINITION_ID, mode: "zk-native", transferVerifyingKey: "halo2/ipa:vk_transfer" })],
    ["zk.Shield", buildShieldInstruction({ assetDefinitionId: ASSET_DEFINITION_ID, fromAccountId: ACCOUNT_ID, amount: "7", noteCommitment: Buffer.alloc(32, 0x01), encryptedPayload: { version: 1, ephemeralPublicKey: Buffer.alloc(32, 0x02), nonce: Buffer.alloc(24, 0x03), ciphertext: Buffer.from("ciphertext") } })],
    [
      "zk.ZkTransfer",
      buildZkTransferInstruction({
        assetDefinitionId: ASSET_DEFINITION_ID,
        inputs: [Buffer.alloc(32, 0x11)],
        outputs: [Buffer.alloc(32, 0x22)],
        proof: {
          backend: "halo2/ipa",
          proof: Buffer.from("proof"),
          verifyingKeyRef: "halo2/ipa:vk_transfer",
          verifyingKeyCommitment: Buffer.alloc(32, 0x33),
          envelopeHash: Buffer.alloc(32, 0x44),
        },
        rootHint: Buffer.alloc(32, 0x55),
      }),
    ],
    [
      "zk.Unshield",
      buildUnshieldInstruction({
        assetDefinitionId: ASSET_DEFINITION_ID,
        toAccountId: SAMPLE_ACCOUNT_ID,
        publicAmount: "3",
        inputs: [Buffer.alloc(32, 0x66)],
        proof: {
          backend: "halo2/ipa",
          proof: Buffer.from("proof-unshield"),
          verifyingKeyRef: "halo2/ipa:vk_unshield",
        },
        rootHint: Buffer.alloc(32, 0x77),
      }),
    ],
    ["zk.FinalizeElection", buildFinalizeElectionInstruction({ electionId: "elec-1", tally: [1, "2"], tallyProof: { backend: "halo2/ipa", proof: Buffer.from("proof"), verifyingKeyRef: "halo2/ipa:vk_tally" } })],
    ["Rwa.RegisterRwa", buildRegisterRwaInstruction({ rwa: { domain: "commodities", quantity: "10.5", spec: { scale: 1 }, primaryReference: "vault-cert-001", metadata: { origin: "AE" }, parents: [{ rwa: RWA_ID, quantity: "1.25" }], controls: { controllerAccounts: [ACCOUNT_ID], freezeEnabled: true } } })],
    ["Rwa.SetRwaControls", buildSetRwaControlsInstruction({ rwaId: RWA_ID, controls: { redeemEnabled: true } })],
    ["Rwa.SetRwaKeyValue", buildSetRwaKeyValueInstruction({ rwaId: RWA_ID, key: "grade", value: { country: "AE", sequence: BigInt(7) } })],
  ];
}

test("JS-only norito fallback matches native bytes across the broader instruction surface", async () => {
  await withDisabledNative(async () => {
    const jsMod = await import("../src/norito.js?js-fallback-matrix");
    for (const [name, instruction] of representativeInstructions()) {
      const nativeEncoded = nativeEncodeInstruction(instruction);
      const jsEncoded = jsMod.noritoEncodeInstruction(instruction);
      const nativeDecoded = nativeDecodeInstruction(nativeEncoded);
      assert.equal(jsEncoded.toString("hex"), nativeEncoded.toString("hex"), name);
      assert.deepEqual(jsMod.noritoDecodeInstruction(nativeEncoded), nativeDecoded, name);
      assert.equal(
        jsMod.noritoEncodeInstruction(jsMod.noritoDecodeInstruction(nativeEncoded)).toString("hex"),
        nativeEncoded.toString("hex"),
        `${name} re-encode`,
      );
    }
  });
});

test("JS-only decode works in a fresh process without the round-trip cache", async () => {
  for (const [name, instruction] of representativeInstructions()) {
    const nativeEncoded = nativeEncodeInstruction(instruction);
    const nativeDecoded = nativeDecodeInstruction(nativeEncoded);
    const fresh = decodeInFreshJsOnlyProcess(nativeEncoded);
    assert.deepEqual(fresh.decoded, nativeDecoded, `${name} fresh decode`);
    assert.equal(fresh.reencodedHex, nativeEncoded.toString("hex"), `${name} fresh re-encode`);
  }
});

test("JS-only Custom aliases encode successfully and decode canonically", async () => {
  await withDisabledNative(async () => {
    const jsMod = await import("../src/norito.js?js-fallback-custom-aliases");
    const spec = sampleSpec();
    const specPayload = spec.toPayload();
    const proposalPayload = {
      account: ACCOUNT_ID,
      instructions: [{ ExecuteTrigger: { trigger: "staged_mint_request_hbl", args: { request_id: "mr-1" } } }],
      transaction_ttl_ms: 45_000,
    };
    const aliases = [
      [
        "Multisig",
        { Multisig: { Register: { account: ACCOUNT_ID, spec } } },
        { Custom: { payload: { Register: { account: ACCOUNT_ID, spec: specPayload } } } },
      ],
      [
        "MultisigRegister",
        { MultisigRegister: { account: ACCOUNT_ID, spec } },
        { Custom: { payload: { Register: { account: ACCOUNT_ID, spec: specPayload } } } },
      ],
      [
        "MultisigPropose",
        { MultisigPropose: proposalPayload },
        { Custom: { payload: { Propose: proposalPayload } } },
      ],
      [
        "MultisigApprove",
        { MultisigApprove: { account: ACCOUNT_ID, proposal_id: "proposal-1" } },
        { Custom: { payload: { Approve: { account: ACCOUNT_ID, proposal_id: "proposal-1" } } } },
      ],
      [
        "MultisigCancel",
        { MultisigCancel: { account: ACCOUNT_ID, proposal_id: "proposal-2" } },
        { Custom: { payload: { Cancel: { account: ACCOUNT_ID, proposal_id: "proposal-2" } } } },
      ],
    ];

    for (const [name, aliasInstruction, canonicalInstruction] of aliases) {
      const aliasBytes = jsMod.noritoEncodeInstruction(aliasInstruction);
      const canonicalBytes = jsMod.noritoEncodeInstruction(canonicalInstruction);
      assert.equal(aliasBytes.toString("hex"), canonicalBytes.toString("hex"), `${name} bytes`);
      assert.deepEqual(
        jsMod.noritoDecodeInstruction(aliasBytes),
        canonicalInstruction,
        `${name} decode`,
      );
    }
  });
});

test("JS-only public-key multihash path matches native for enabled curve families", async () => {
  const curveCases = [
    ["ml-dsa", ML_DSA_PUBLIC_KEY],
    ["gost256a", GOST256A_PUBLIC_KEY],
    ["sm2", Buffer.from(hexToBytes("04361255A512347E76EA947EBB416C12D4C07E30B150C0EC2047ECC5E142907499B8D99C4C5CF69BFF6527E7B67396B55E42EF98625B339696DBEF9A3AABBFC06F"))],
  ];

  configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true });
  try {
    await withDisabledNative(async () => {
      const jsMod = await import("../src/norito.js?js-fallback-curves");
      for (const [algorithm, publicKey] of curveCases) {
        const destination = destinationAccountIdForCurve(algorithm, publicKey);
        const instruction = buildTransferDomainInstruction({
          sourceAccountId: ACCOUNT_ID,
          domainId: "wonderland",
          destinationAccountId: destination,
        });
        const nativeEncoded = nativeEncodeInstruction(instruction);
        const jsEncoded = jsMod.noritoEncodeInstruction(instruction);
        assert.equal(jsEncoded.toString("hex"), nativeEncoded.toString("hex"), algorithm);
      }

      const multisigAccountId = AccountAddress.fromCanonicalBytes(
        hexToBytes(MULTISIG_CANONICAL_HEX),
      ).toI105();
      const multisigInstruction = buildTransferDomainInstruction({
        sourceAccountId: ACCOUNT_ID,
        domainId: "wonderland",
        destinationAccountId: multisigAccountId,
      });
      const nativeEncoded = nativeEncodeInstruction(multisigInstruction);
      const jsEncoded = jsMod.noritoEncodeInstruction(multisigInstruction);
      assert.equal(jsEncoded.toString("hex"), nativeEncoded.toString("hex"), "real multisig");
    });
  } finally {
    configureCurveSupport();
  }
});
