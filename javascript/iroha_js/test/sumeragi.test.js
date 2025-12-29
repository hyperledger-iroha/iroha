import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { ToriiClient } from "../src/toriiClient.js";

describe("sumeragi endpoints", () => {
  it("lists commit certificates and key lifecycle entries", async () => {
    const client = new ToriiClient("http://localhost");
    client._request = async (_method, path) => {
      if (path === "/v1/sumeragi/commit-certificates") {
        return [{ height: 1 }];
      }
      if (path === "/v1/sumeragi/key-lifecycle") {
        return [{ public_key: "pk" }];
      }
      throw new Error(`unexpected path ${path}`);
    };

    const certs = await client.listSumeragiCommitCertificates();
    assert.deepEqual(certs, [{ height: 1 }]);
    const keys = await client.listSumeragiKeyLifecycle();
    assert.deepEqual(keys, [{ public_key: "pk" }]);
  });
});
