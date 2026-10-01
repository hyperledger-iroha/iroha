import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { ed25519 } from "@noble/curves/ed25519";
import { AccountAddress } from "../src/address.js";
import { NetworkId } from "../src/networkId.js";
import { LocalSigningContext, ToriiClient } from "../src/toriiClient.js";
import { canonicalHashLiteral } from "../src/instructionBuilderPrimitives.js";
import {
  noritoEncodeInstruction, noritoDecodeInstruction,
  noritoEncodeInstructionBoxArchive, noritoDecodeInstructionBoxArchive,
} from "../src/norito.js";
import {
  buildRegisterSmartContractCodeInstruction, buildRegisterSmartContractBytesInstruction,
  buildUploadSmartContractCodeChunkInstruction, buildFinalizeSmartContractCodeUploadInstruction,
  buildCancelSmartContractCodeUploadInstruction, buildRemoveSmartContractBytesInstruction,
} from "../src/instructionBuilders.js";

const fixture = JSON.parse(readFileSync(new URL("./fixtures/current_rust_contract_artifact.json", import.meta.url), "utf8"));
const hash = fixture.artifact_semantics.code_hash_hex;
const dataspace = "18446744073709551615";
const artifactId = { dataspaceId: dataspace, codeHash: hash };
const networkId = NetworkId.fromBytes(Buffer.alloc(32, 0xa5));
const privateKey = Buffer.alloc(32, 0x0b);
const accountId = AccountAddress.fromAccount({ algorithm: "ed25519", publicKey: ed25519.getPublicKey(privateKey) }).toI105(753);
const auth = { canonicalAuth: { accountId, privateKey } };
const wireId = { dataspace_id: dataspace, code_hash: canonicalHashLiteral(Buffer.from(hash, "hex")) };

function clientFor(payload, calls = []) {
  return new ToriiClient("https://torii.example", {
    localSigningContext: new LocalSigningContext(networkId, 753),
    fetchImpl: async (url, init) => {
      calls.push({ url, init });
      const text = JSON.stringify(payload).replace(`"dataspace_id":"${dataspace}"`, `"dataspace_id":${dataspace}`);
      return new Response(text, { status: 200, headers: { "content-type": "application/json" } });
    },
  });
}

test("six artifact instructions require one exact full-width dataspace", () => {
  const inputs = [
    [buildRegisterSmartContractCodeInstruction, { manifest: fixture.manifest }],
    [buildRegisterSmartContractBytesInstruction, { code: Buffer.of(1) }],
    [buildUploadSmartContractCodeChunkInstruction, { totalSize: 1, chunkIndex: 0, chunkCount: 1, chunk: Buffer.of(1) }],
    [buildFinalizeSmartContractCodeUploadInstruction, { totalSize: 1, chunkCount: 1 }],
    [buildCancelSmartContractCodeUploadInstruction, {}],
    [buildRemoveSmartContractBytesInstruction, {}],
  ];
  for (const [build, input] of inputs) {
    const instruction = build({ ...input, artifactId });
    const [variant] = Object.keys(instruction);
    assert.deepEqual(instruction[variant].artifact_id, wireId);
    for (const [encode, decode] of [
      [noritoEncodeInstruction, noritoDecodeInstruction],
      [noritoEncodeInstructionBoxArchive, noritoDecodeInstructionBoxArchive],
    ]) {
      const bytes = encode(instruction, 753);
      assert.deepEqual(decode(bytes, 753), instruction);
      const universal = build({ ...input, artifactId: { ...artifactId, dataspaceId: "0" } });
      assert.notDeepEqual(encode(universal, 753), bytes);
      const retired = structuredClone(instruction);
      delete retired[variant].artifact_id;
      retired[variant].code_hash = wireId.code_hash;
      assert.throws(() => encode(retired, 753));
      assert.throws(() => encode({ ...instruction, Mint: null }, 753));
      const extra = structuredClone(instruction);
      extra[variant].artifact_id.unexpected = null;
      assert.throws(() => encode(extra, 753));
      const numeric = structuredClone(instruction);
      numeric[variant].artifact_id.dataspace_id = 0;
      assert.throws(() => encode(numeric, 753));
    }
    assert.throws(() => build({ ...input, codeHash: hash }));
    for (const invalid of [-1, Number.MAX_SAFE_INTEGER + 1, "01", "18446744073709551616"]) {
      assert.throws(() => build({ ...input, artifactId: { ...artifactId, dataspaceId: invalid } }));
    }
  }
});

test("artifact reads require account authentication and bind network, u64 scope and bytes", async () => {
  const payload = { network_id: networkId.literal, artifact_id: wireId, code_b64: fixture.artifact_base64 };
  const calls = [];
  const client = clientFor(payload, calls);
  await assert.rejects(client.getContractCodeBytes(artifactId), /canonicalAuth/u);
  assert.equal(calls.length, 0);
  const result = await client.getContractCodeBytes(artifactId, auth);
  assert.deepEqual(result.artifact_id, wireId);
  assert.equal(calls[0].url, `https://torii.example/v1/contracts/artifacts/${dataspace}/${hash}/bytes`);
  assert.ok(Object.keys(calls[0].init.headers).some((key) => key.toLowerCase() === "x-iroha-signature"));
  for (const replacement of [
    { ...payload, network_id: NetworkId.fromBytes(Buffer.alloc(32, 0xa7)).literal },
    { ...payload, artifact_id: { ...wireId, dataspace_id: "7" } },
    { ...payload, artifact_id: { ...wireId, code_hash: canonicalHashLiteral(Buffer.alloc(32, 0xab)) } },
    { ...payload, code_b64: Buffer.of(1).toString("base64") },
  ]) {
    await assert.rejects(clientFor(replacement).getContractCodeBytes(artifactId, auth));
  }
  assert.equal(typeof client.registerContractCode, "undefined");
});
