import assert from "node:assert/strict";
import test from "node:test";

import { computeHashLiteralCrc } from "../src/hashLiteralCrc.js";

test("hash literal CRC matches canonical NetworkId vectors", () => {
  assert.equal(
    computeHashLiteralCrc(
      "hash",
      "32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149",
    ),
    "A2F0",
  );
});
