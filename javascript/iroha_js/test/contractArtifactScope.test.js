import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { ed25519 } from "@noble/curves/ed25519";
import { AccountAddress } from "../src/address.js";
import { NetworkId } from "../src/networkId.js";
import { LocalSigningContext, ToriiClient } from "../src/toriiClient.js";
import { canonicalHashLiteral } from "../src/instructionBuilderPrimitives.js";
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
function artifactAuth() {
  const accountId = AccountAddress.fromAccount({ algorithm: "ed25519", publicKey: ed25519.getPublicKey(privateKey) }).toI105(753);
  return { canonicalAuth: { accountId, privateKey } };
}
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
    assert.deepEqual(Object.values(build({ ...input, artifactId }))[0].artifact_id, wireId);
    assert.throws(() => build({ ...input, codeHash: hash }));
    for (const invalid of [-1, Number.MAX_SAFE_INTEGER + 1, "01", "18446744073709551616"]) {
      assert.throws(() => build({ ...input, artifactId: { ...artifactId, dataspaceId: invalid } }));
    }
  }
});

test("artifact reads require account authentication and bind network, u64 scope and bytes", async () => {
  const auth = artifactAuth();
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

test("artifact response normalization preserves full-u64 identity and rejects scope substitution", async () => {
  // The native signed transport is tested above; this test independently exercises
  // bounded response admission using an already authenticated transport result.
  const auth = { canonicalAuth: { accountId: "alice-1@wonderland", privateKey } };
  const payload = { network_id: networkId.literal, artifact_id: wireId, code_b64: fixture.artifact_base64 };
  const normalizedClient = (record) => {
    const client = clientFor(record);
    client._request = async () => new Response(
      JSON.stringify(record).replace(`"dataspace_id":"${dataspace}"`, `"dataspace_id":${dataspace}`),
      { headers: { "content-type": "application/json" } },
    );
    return client;
  };
  await assert.rejects(normalizedClient(payload).getContractCodeBytes(artifactId), /canonicalAuth/u);
  const result = await normalizedClient(payload).getContractCodeBytes(artifactId, auth);
  assert.deepEqual(result, payload);
  for (const replacement of [
    { ...payload, network_id: NetworkId.fromBytes(Buffer.alloc(32, 0xa7)).literal },
    { ...payload, artifact_id: { ...wireId, dataspace_id: "7" } },
    { ...payload, artifact_id: { ...wireId, code_hash: canonicalHashLiteral(Buffer.alloc(32, 0xab)) } },
    { ...payload, artifact_id: { ...wireId, dataspace_id: "18446744073709551616" } },
    { ...payload, artifact_id: { ...wireId, dataspace_id: "01" } },
    { ...payload, code_b64: `${fixture.artifact_base64}\n` },
    { ...payload, code_b64: Buffer.of(1).toString("base64") },
    { ...payload, extra: true },
    { code_b64: fixture.artifact_base64 },
  ]) await assert.rejects(normalizedClient(replacement).getContractCodeBytes(artifactId, auth));
  const manifestResponse = { network_id: networkId.literal, artifact_id: wireId, manifest: fixture.manifest };
  const manifest = await normalizedClient(manifestResponse).getContractManifest(artifactId, auth);
  assert.deepEqual(manifest.artifact_id, wireId);
  assert.equal(manifest.code_hash, hash);
  const inline = await normalizedClient({ ...manifestResponse, code_bytes: fixture.artifact_base64 }).getContractManifest(artifactId, auth);
  assert.equal(inline.code_bytes, fixture.artifact_base64);
  for (const replacement of [
    { ...manifestResponse, artifact_id: { ...wireId, dataspace_id: "0" } },
    { ...manifestResponse, network_id: NetworkId.fromBytes(Buffer.alloc(32, 0xa7)).literal },
    { ...manifestResponse, code_bytes: null },
    { ...manifestResponse, extra: true },
  ]) await assert.rejects(normalizedClient(replacement).getContractManifest(artifactId, auth));
});
