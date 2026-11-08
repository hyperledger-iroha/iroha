// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

// Server-side rendering imports the widget without a DOM.

import assert from "node:assert/strict";
import test from "node:test";

test("importing the widget without a DOM registers nothing and never throws", async () => {
  assert.equal(globalThis.customElements, undefined);
  const widget = await import("../src/index.js");
  const sakura = await import("../src/sakura-element.js");
  assert.equal(widget.defineIrohaConnect(), false);
  assert.equal(sakura.defineIrohaSakura(), false);
  const lease = widget.acquireSakura();
  assert.equal(lease.element, null);
  assert.equal(lease.burst(0, 0, 5), 0);
  lease.release();
  assert.match(widget.IROHA_MARK_SVG, /^<svg /u);
  assert.equal(widget.encodeQr("https://taira.sora.org").version, 2);
});
