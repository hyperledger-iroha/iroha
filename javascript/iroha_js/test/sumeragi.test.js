import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { NetworkId } from "../src/networkId.js";
import { OperatorSigningContext, ToriiClient } from "../src/toriiClient.js";

describe("sumeragi endpoints", () => {
  it("lists key lifecycle entries", async () => {
    const operatorSigningContext = new OperatorSigningContext(
      NetworkId.fromBytes(new Uint8Array(32).fill(1)),
      {
        publicKey: "test-key",
        sign: () => new Uint8Array(64).fill(1),
      },
    );
    const client = new ToriiClient("http://localhost", { operatorSigningContext });
    client._request = async (_method, path) => {
      if (path === "/v1/sumeragi/key-lifecycle") {
        return [{ public_key: "pk" }];
      }
      throw new Error(`unexpected path ${path}`);
    };

    const keys = await client.listSumeragiKeyLifecycle();
    assert.deepEqual(keys, [{ public_key: "pk" }]);
  });
});
