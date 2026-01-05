import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import { decodeReplicationOrder } from "../src/sorafs.js";

const ORDER_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/replication_order/order_v1.to",
  import.meta.url,
);

test("decodeReplicationOrder returns structured replication order", () => {
  const bytes = readFileSync(ORDER_FIXTURE);
  const order = decodeReplicationOrder(bytes);

  assert.equal(order.schemaVersion, 1);
  assert.equal(
    order.orderIdHex,
    "abababababababababababababababababababababababababababababababab",
  );
  assert.equal(order.manifestCidUtf8, "bafyreplicaexamplecidroot");
  assert.equal(
    order.manifestDigestHex,
    "4242424242424242424242424242424242424242424242424242424242424242",
  );
  assert.equal(order.chunkingProfile, "sorafs.sf1@1.0.0");
  assert.equal(order.targetReplicas, 2);
  assert.equal(order.assignments.length, 2);
  assert.equal(
    order.assignments[0]?.providerIdHex,
    "1010101010101010101010101010101010101010101010101010101010101010",
  );
  assert.equal(order.assignments[0]?.sliceGiB, 512);
  assert.equal(order.assignments[0]?.lane, "lane-primary");
  assert.equal(
    order.assignments[1]?.providerIdHex,
    "1111111111111111111111111111111111111111111111111111111111111111",
  );
  assert.equal(order.assignments[1]?.sliceGiB, 512);
  assert.equal(order.assignments[1]?.lane, "lane-secondary");
  assert.equal(order.issuedAtUnix, 1_700_000_000);
  assert.equal(order.deadlineAtUnix, 1_700_086_400);
  assert.deepEqual(order.sla, {
    ingestDeadlineSecs: 86_400,
    minAvailabilityPercentMilli: 99_500,
    minPorSuccessPercentMilli: 98_000,
  });
  assert.equal(order.metadata.length, 1);
  assert.deepEqual(order.metadata[0], {
    key: "governance.ticket",
    value: "ticket-sorafs-0001",
  });
});

test("decodeReplicationOrder accepts header padding", () => {
  const bytes = readFileSync(ORDER_FIXTURE);
  const headerLen = 40;
  const padding = Buffer.alloc(8);
  const padded = Buffer.concat([
    bytes.subarray(0, headerLen),
    padding,
    bytes.subarray(headerLen),
  ]);
  const order = decodeReplicationOrder(padded);
  assert.equal(order.schemaVersion, 1);
  assert.equal(order.orderIdHex.length, 64);
});

test("decodeReplicationOrder rejects payloads without a Norito header", () => {
  assert.throws(
    () => decodeReplicationOrder(Buffer.alloc(8)),
    /header|payload is too small/i,
  );
});
