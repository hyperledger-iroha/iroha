import assert from "node:assert/strict";
import { test } from "node:test";

import { createVerifyingKeyClient } from "../src/verifyingKeyClient.js";
import { createVerifyingKeyClient as createDistVerifyingKeyClient } from "../dist/verifyingKeyClient.js";
import { ToriiClient } from "../src/toriiClient.js";
import { ToriiClient as DistToriiClient } from "../dist/toriiClient.js";

const current = "halo2/pasta/ivm-replay-binding-v1";
const retired = "halo2/pasta/ivm-execution-v1";

test("source and packaged verifier registries accept only the replay binding label", () => {
  for (const create of [createVerifyingKeyClient, createDistVerifyingKeyClient]) {
    // The backend validator has no dependency on address codecs or native proof
    // execution. Exercise the actual validator without a mocked native addon.
    const registry = create();
    assert.equal(registry.backend(current, "backend"), current);
    for (const backend of [retired, `${current}/`, current.toUpperCase(), `${current}\0`]) {
      assert.throws(() => registry.backend(backend, "backend"), /unsupported production verifier backend/);
    }
  }
});

test("source and packaged Torii reject retired IVM backend before transport", async () => {
  for (const Client of [ToriiClient, DistToriiClient]) {
    let calls = 0;
    const client = new Client("https://localhost:8080", {
      fetchImpl: async (url) => {
        calls += 1;
        assert.equal(new URL(url).searchParams.get("backend"), current);
        return new Response(JSON.stringify([]), {
          status: 200,
          headers: { "content-type": "application/json" },
        });
      },
    });
    await assert.rejects(() => client.listVerifyingKeys({ backend: retired }), /unsupported production verifier backend/);
    assert.equal(calls, 0);
    await client.listVerifyingKeys({ backend: current });
    assert.equal(calls, 1);
  }
});
