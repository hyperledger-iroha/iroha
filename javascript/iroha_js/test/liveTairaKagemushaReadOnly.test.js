import { test } from "node:test";
import assert from "node:assert/strict";

import { TAIRA_TESTNET_PROFILE, ToriiClient } from "../src/index.js";

const enabled = process.env.IROHA_TAIRA_KAGEMUSHA_READ_ONLY === "1";

function requireCredentialFreeHttpsOrigin(raw) {
  if (raw !== raw.trim()) {
    throw new TypeError(
      "IROHA_TAIRA_PUBLIC_ROOT must be a credential-free HTTPS origin without a path, query, or fragment",
    );
  }
  let origin;
  try {
    origin = new URL(raw);
  } catch {
    throw new TypeError(
      "IROHA_TAIRA_PUBLIC_ROOT must be a credential-free HTTPS origin without a path, query, or fragment",
    );
  }
  if (
    origin.protocol !== "https:"
    || /^https:\/\/[^/?#]*@/iu.test(raw)
    || origin.username !== ""
    || origin.password !== ""
    || origin.hostname === ""
    || origin.pathname !== "/"
    || origin.search !== ""
    || origin.hash !== ""
  ) {
    throw new TypeError(
      "IROHA_TAIRA_PUBLIC_ROOT must be a credential-free HTTPS origin without a path, query, or fragment",
    );
  }
  return origin.origin;
}

test("Taira probe accepts a credential-free HTTPS origin", () => {
  assert.equal(
    requireCredentialFreeHttpsOrigin("https://taira.sora.org/"),
    "https://taira.sora.org",
  );
});

test("Taira probe rejects non-origin overrides", () => {
  for (const value of [
    " http://taira.sora.org",
    "http://taira.sora.org",
    "https://@taira.sora.org",
    "https://user@taira.sora.org",
    "https://taira.sora.org/v1",
    "https://taira.sora.org?query=1",
    "https://taira.sora.org#fragment",
  ]) {
    assert.throws(() => requireCredentialFreeHttpsOrigin(value), /credential-free HTTPS origin/u);
  }
});

test(
  "public Taira exposes the exact Kagemusha capability to the JavaScript SDK",
  { skip: !enabled, timeout: 30_000 },
  async () => {
    const publicRoot = requireCredentialFreeHttpsOrigin(
      process.env.IROHA_TAIRA_PUBLIC_ROOT ?? TAIRA_TESTNET_PROFILE.toriiBaseUrl,
    );
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
