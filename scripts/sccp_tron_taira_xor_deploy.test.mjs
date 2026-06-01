#!/usr/bin/env node
// Unit tests for the TAIRA XOR TRON deployment helper's offline validation
// paths. These tests do not contact TRON and must never broadcast.
import assert from "node:assert/strict";
import test from "node:test";
import { secp256k1 } from "../javascript/iroha_js/node_modules/@noble/curves/secp256k1.js";
import { sha256 } from "../javascript/iroha_js/node_modules/@noble/hashes/sha256.js";
import {
  TRON_MAINNET_NETWORK_ID_HEX,
  bytesToHex,
  compileTairaBurnRecordContract,
  hexToBytes,
  normalizeTronAddress,
  normalizeTronBase58Address,
  normalizeVerifierConstructorArgs,
  routeHash,
  signTransactionPayload,
  tronAddressFromPrivateKey,
} from "./sccp_tron_taira_xor_deploy.mjs";

const privateKey = new Uint8Array(32).fill(7);
const deployerAddress = tronAddressFromPrivateKey(privateKey);

const mockTransaction = (ownerAddress = deployerAddress.base58, rawData = [1, 2, 3, 4]) => {
  const rawBytes = Uint8Array.from(rawData);
  return {
    visible: true,
    txID: bytesToHex(sha256(rawBytes), false),
    raw_data: {
      contract: [
        {
          parameter: {
            value: {
              owner_address: ownerAddress,
            },
          },
          type: "TriggerSmartContract",
        },
      ],
      timestamp: 1,
      expiration: 2,
    },
    raw_data_hex: bytesToHex(rawBytes, false),
  };
};

const verifierMaterial = () => ({
  alpha1: [1, 2],
  beta2: [
    [3, 4],
    [5, 6],
  ],
  gamma2: [7, 8, 9, 10],
  delta2: [11, 12, 13, 14],
  ic: [15, 16, 17, 18],
  verifierKeyHash: routeHash("verifier-key"),
});

test("normalizes canonical TRON Base58, 21-byte hex, and Solidity address forms", () => {
  const fromBase58 = normalizeTronBase58Address(deployerAddress.base58, "base58");
  assert.equal(fromBase58.hex, deployerAddress.hex);
  assert.equal(fromBase58.solidity, `0x${deployerAddress.hex.slice(4)}`);

  const fromHex = normalizeTronAddress(deployerAddress.hex, "hex");
  assert.equal(fromHex.base58, deployerAddress.base58);
  assert.equal(fromHex.solidity, fromBase58.solidity);

  const fromSolidity = normalizeTronAddress(fromBase58.solidity, "solidity");
  assert.equal(fromSolidity.base58, deployerAddress.base58);
});

test("rejects malformed, non-canonical, checksum-invalid, and zero TRON addresses", () => {
  assert.throws(() => normalizeTronBase58Address(` ${deployerAddress.base58}`, "address"), /canonical/);
  assert.throws(
    () =>
      normalizeTronBase58Address(
        `${deployerAddress.base58.slice(0, -1)}${deployerAddress.base58.endsWith("1") ? "2" : "1"}`,
        "address",
      ),
    /checksum/,
  );
  assert.throws(() => normalizeTronAddress("0x410000000000000000000000000000000000000000", "address"), /non-zero/);
  assert.throws(() => normalizeTronAddress("not-an-address", "address"), /hex|Base58/);
});

test("offline signing binds txID, raw_data owner, and recovered signer", () => {
  const transaction = mockTransaction();
  const signed = signTransactionPayload(transaction, { privateKey, address: deployerAddress });
  assert.equal(signed.metadata.txid, transaction.txID);
  assert.equal(signed.metadata.signature_recovers_to_owner, true);
  assert.equal(signed.metadata.signature_recovered_base58, deployerAddress.base58);
  assert.match(signed.signed.signature[0], /^[0-9a-f]{130}$/u);
});

test("offline signing rejects txid mismatch, wrong owner, duplicate signatures, and malformed owners", () => {
  assert.throws(
    () =>
      signTransactionPayload(
        {
          ...mockTransaction(),
          txID: "00".repeat(32),
        },
        { privateKey, address: deployerAddress },
      ),
    /txID/,
  );

  const otherAddress = tronAddressFromPrivateKey(new Uint8Array(32).fill(8));
  assert.throws(
    () => signTransactionPayload(mockTransaction(otherAddress.base58), { privateKey, address: deployerAddress }),
    /does not match deployer/,
  );

  assert.throws(
    () =>
      signTransactionPayload(
        {
          ...mockTransaction(),
          signature: ["00".repeat(65)],
        },
        { privateKey, address: deployerAddress },
      ),
    /already signed/,
  );

  assert.throws(
    () => signTransactionPayload(mockTransaction("bad-owner"), { privateKey, address: deployerAddress }),
    /hex|Base58/,
  );
});

test("verifier material normalization accepts production lane material", () => {
  const args = normalizeVerifierConstructorArgs(verifierMaterial());
  assert.equal(args.length, 10);
  assert.deepEqual(args[0], ["1", "2"]);
  assert.equal(args[5], routeHash("verifier-key"));
  assert.equal(args[6], "stark-fri-v1");
  assert.equal(args[7], TRON_MAINNET_NETWORK_ID_HEX);
  assert.equal(args[8], 0);
  assert.equal(args[9], 5);
});

test("verifier material normalization rejects stale or adversarial lane material", () => {
  assert.throws(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial(), proofFamily: "debug-proof-family" }),
    /proofFamily/,
  );
  assert.throws(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial(), networkId: routeHash("wrong-network") }),
    /networkId/,
  );
  assert.throws(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial(), sourceDomain: 5, targetDomain: 0 }),
    /SORA -> TRON/,
  );
  assert.throws(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial(), beta2: [1, 2, 3] }),
    /beta2/,
  );
  assert.throws(
    () => normalizeVerifierConstructorArgs({ ...verifierMaterial(), verifierKeyHash: "0x00" }),
    /32 bytes/,
  );
});

test("hex helper rejects odd, non-hex, and wrong-length values", () => {
  assert.deepEqual([...hexToBytes("0x0102", "sample", 2)], [1, 2]);
  assert.throws(() => hexToBytes("0x1", "sample"), /even-length/);
  assert.throws(() => hexToBytes("0xzz", "sample"), /hex/);
  assert.throws(() => hexToBytes("0x0102", "sample", 3), /3 bytes/);
});

test("generated deployer private key still derives a valid TRON address shape", () => {
  const generated = tronAddressFromPrivateKey(secp256k1.utils.randomPrivateKey());
  assert.match(generated.base58, /^T[1-9A-HJ-NP-Za-km-z]{33}$/u);
  assert.match(generated.hex, /^0x41[0-9a-f]{40}$/u);
});

test("TAIRA burn-record contract artifact is compiled with forced IVM ZK mode", async () => {
  const contract = await compileTairaBurnRecordContract();
  assert.equal(contract.schema, "iroha-sccp-taira-xor-burn-record-contract/v1");
  assert.equal(contract.route_id, "taira_tron_xor");
  assert.equal(contract.asset_key, "xor");
  assert.match(contract.code_hash, /^[0-9a-f]{64}$/u);
  assert.match(contract.artifact_sha256, /^0x[0-9a-f]{64}$/u);
  assert.equal(contract.execution.executable, "IvmProved");
  assert.equal(contract.execution.force_zk_mode, true);
  assert.equal(contract.execution.entrypoint, "burn_and_record");
  assert.equal(contract.manifest?.features_bitmap, 1);
});
