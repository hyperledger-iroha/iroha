import { test } from "node:test";
import assert from "node:assert/strict";

import { ToriiClient } from "../src/index.js";

const enabled = process.env.IROHA_TAIRA_KAGEMUSHA_READ_ONLY === "1";
const publicRoot = process.env.IROHA_TAIRA_PUBLIC_ROOT ?? "https://taira.sora.org";

test(
  "public Taira exposes the exact Kagemusha capability to the JavaScript SDK",
  { skip: !enabled, timeout: 30_000 },
  async () => {
    const capability = await new ToriiClient(publicRoot, { maxRetries: 0 })
      .getOfflineCapability({ signal: AbortSignal.timeout(20_000) });
    assert.deepEqual(capability, {
      cash_handoff_capability: "cash_handoff_v1",
      required_bridge_abi_version: 23,
      max_hops: 8,
      ready: true,
    });
  },
);
