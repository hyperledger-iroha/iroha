import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1,
  buildCompleteReplicationOrderInstruction,
  buildExpireReplicationOrderInstruction,
  buildIssueReplicationOrderInstruction,
} from "../src/instructionBuilders.js";
import {
  noritoDecodeInstruction,
  noritoEncodeInstruction,
  validateSorafsReplicationOrderPayloadV1,
} from "../src/norito.js";

const FIXTURE_ROOT = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../../fixtures/sorafs_manifest/replication_order",
);
const ORDER_PAYLOAD = fs.readFileSync(path.join(FIXTURE_ROOT, "order_v1.to"));
const ORDER_PAYLOAD_BASE64 = ORDER_PAYLOAD.toString("base64");
const ORDER_ID = "2b".repeat(32);
const MUSUBI_ARCHIVE_ID = "cd".repeat(32);
const PROVIDER_ID = "10".repeat(32);
const PROVIDER_OWNER =
  "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
const POLICY_ID = "21".repeat(32);
const PREDECESSOR_DIGEST = "32".repeat(32);
const POLICY_DIGEST = "43".repeat(32);
const BLOCK_HASH = "54".repeat(32);
const CRC64_POLY = 0xc96c5795d7870f42n;
const U64_MASK = 0xffff_ffff_ffff_ffffn;

function completionOptions(overrides = {}) {
  return {
    orderId: ORDER_ID,
    providerId: PROVIDER_ID,
    completionEpoch: 27,
    expectedAuthority: {
      providerOwner: PROVIDER_OWNER,
      signerPolicy: {
        policyId: POLICY_ID,
        revision: 2,
        predecessorDigest: PREDECESSOR_DIGEST,
        policyDigest: POLICY_DIGEST,
      },
    },
    expectedAssignmentRevision: 3,
    finalizedAnchor: {
      height: 41,
      blockHash: BLOCK_HASH,
    },
    ...overrides,
  };
}

function crc64(payload) {
  let crc = U64_MASK;
  for (const byte of payload) {
    crc ^= BigInt(byte);
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc & 1n) === 1n ? (crc >> 1n) ^ CRC64_POLY : crc >> 1n;
    }
  }
  return BigInt.asUintN(64, crc ^ U64_MASK);
}

function mutatePayload(mutator) {
  const mutated = Buffer.from(ORDER_PAYLOAD);
  mutator(mutated);
  mutated.writeBigUInt64LE(crc64(mutated.subarray(40)), 31);
  return mutated.toString("base64");
}

function replaceUnique(buffer, needle, replacement) {
  const offset = buffer.indexOf(needle, 40);
  assert.notEqual(offset, -1, `fixture must contain ${needle.toString("hex")}`);
  assert.equal(buffer.indexOf(needle, offset + 1), -1, "fixture marker must be unique");
  replacement.copy(buffer, offset);
}

test("SoraFS replication instruction builders emit canonical native field names", () => {
  const summary = validateSorafsReplicationOrderPayloadV1(ORDER_PAYLOAD, ORDER_ID);
  assert.equal(summary.manifestDigestHex, "42".repeat(32));
  assert.equal(
    summary.manifestCidBase64,
    Buffer.from(`01711f20${"41".repeat(32)}`, "hex").toString("base64"),
  );
  assert.equal(summary.chunkingProfile, "sorafs.sf1@1.0.0");
  assert.deepEqual(summary.assignments, [
    {
      providerIdHex: "10".repeat(32),
      sliceGiB: "512",
      lane: "lane-primary",
    },
    {
      providerIdHex: "11".repeat(32),
      sliceGiB: "512",
      lane: "lane-secondary",
    },
  ]);
  assert.deepEqual(summary.sla, {
    ingestDeadlineSecs: 86_400,
    minAvailabilityPercentMilli: 99_500,
    minPorSuccessPercentMilli: 98_000,
  });
  assert.deepEqual(summary.metadata, [
    { key: "governance.ticket", value: "ticket-sorafs-0001" },
  ]);

  const issue = buildIssueReplicationOrderInstruction({
    orderId: ORDER_ID,
    orderPayload: ORDER_PAYLOAD_BASE64,
    issuedEpoch: 20,
    deadlineEpoch: 28,
  });
  assert.deepEqual(issue, {
    IssueReplicationOrder: {
      order_id: ORDER_ID,
      order_payload: ORDER_PAYLOAD_BASE64,
      issued_epoch: 20,
      deadline_epoch: 28,
      musubi_archive: null,
    },
  });

  const musubiIssue = buildIssueReplicationOrderInstruction({
    orderId: ORDER_ID,
    orderPayload: ORDER_PAYLOAD_BASE64,
    issuedEpoch: 20,
    deadlineEpoch: 28,
    musubiArchiveId: MUSUBI_ARCHIVE_ID,
  });
  assert.equal(
    musubiIssue.IssueReplicationOrder.musubi_archive,
    MUSUBI_ARCHIVE_ID,
  );
  assert.deepEqual(
    noritoDecodeInstruction(noritoEncodeInstruction(musubiIssue)),
    musubiIssue,
  );

  const complete = buildCompleteReplicationOrderInstruction(completionOptions());
  assert.deepEqual(complete, {
    CompleteReplicationOrder: {
      order_id: ORDER_ID,
      provider_id: PROVIDER_ID,
      completion_epoch: 27,
      expected_authority: {
        provider_owner: PROVIDER_OWNER,
        signer_policy: {
          policy_id: POLICY_ID,
          revision: 2,
          predecessor_digest: PREDECESSOR_DIGEST,
          policy_digest: POLICY_DIGEST,
        },
      },
      expected_assignment_revision: 3,
      finalized_anchor: {
        height: 41,
        block_hash: BLOCK_HASH,
      },
    },
  });
  assert.deepEqual(
    noritoDecodeInstruction(noritoEncodeInstruction(complete)),
    complete,
  );

  const expire = buildExpireReplicationOrderInstruction({
    orderId: ORDER_ID,
    expirationEpoch: 29,
  });
  assert.deepEqual(
    noritoDecodeInstruction(noritoEncodeInstruction(expire)),
    expire,
  );
  assert.deepEqual(
    noritoDecodeInstruction(noritoEncodeInstruction(issue)),
    issue,
  );
});

test("SoraFS replication builders reject identifiers, epochs, legacy completion, and unknown fields", () => {
  assert.throws(
    () =>
      noritoEncodeInstruction({
        IssueReplicationOrder: {
          order_id: ORDER_ID,
          order_payload: ORDER_PAYLOAD_BASE64,
          issued_epoch: 20,
          deadline_epoch: 28,
        },
      }),
    /missing field musubi_archive/,
  );
  assert.throws(
    () =>
      buildIssueReplicationOrderInstruction({
        orderId: ORDER_ID,
        orderPayload: ORDER_PAYLOAD_BASE64,
        issuedEpoch: 20,
        deadlineEpoch: 28,
        musubiArchiveId: "00".repeat(32),
      }),
    /zero identifier/,
  );
  assert.throws(
    () =>
      buildCompleteReplicationOrderInstruction(completionOptions({
        providerId: "00".repeat(32),
      })),
    /zero identifier/,
  );
  assert.throws(
    () =>
      buildCompleteReplicationOrderInstruction(completionOptions({
        orderId: ORDER_ID.toUpperCase(),
      })),
    /lowercase hexadecimal/,
  );
  assert.throws(
    () =>
      buildCompleteReplicationOrderInstruction({
        orderId: ORDER_ID,
        providerId: PROVIDER_ID,
        completionEpoch: 1,
      }),
    /expectedAuthority is required/,
  );
  assert.throws(
    () =>
      buildCompleteReplicationOrderInstruction(completionOptions({
        expectedAuthority: {
          providerOwner: PROVIDER_OWNER,
          signerPolicy: {
            policyId: POLICY_ID,
            revision: 2,
            predecessorDigest: null,
            policyDigest: POLICY_DIGEST,
          },
        },
      })),
    /predecessorDigest is required after revision 1/,
  );
  assert.throws(
    () =>
      buildCompleteReplicationOrderInstruction(completionOptions({
        expectedAuthority: {
          providerOwner: ` ${PROVIDER_OWNER}`,
          signerPolicy: completionOptions().expectedAuthority.signerPolicy,
        },
      })),
    /exact canonical I105 account id/,
  );
  assert.throws(
    () =>
      buildCompleteReplicationOrderInstruction(completionOptions({
        expectedAssignmentRevision: 0,
      })),
    /positive integer/,
  );
  assert.throws(
    () =>
      buildCompleteReplicationOrderInstruction(completionOptions({
        finalizedAnchor: { height: 0, blockHash: BLOCK_HASH },
      })),
    /positive integer/,
  );
  assert.throws(
    () =>
      buildIssueReplicationOrderInstruction({
        orderId: ORDER_ID,
        orderPayload: ORDER_PAYLOAD_BASE64,
        issuedEpoch: 4,
        deadlineEpoch: 4,
      }),
    /greater than issuedEpoch/,
  );
  assert.throws(
    () =>
      buildExpireReplicationOrderInstruction({
        orderId: ORDER_ID,
        expirationEpoch: -1,
      }),
    /non-negative integer/,
  );
  assert.throws(
    () =>
      buildExpireReplicationOrderInstruction({
        orderId: ORDER_ID,
        expirationEpoch: 8,
        authority: "confused-deputy",
      }),
    /authority is not supported/,
  );
  assert.throws(
    () =>
      noritoEncodeInstruction({
        CompleteReplicationOrder: {
          order_id: ORDER_ID,
          provider_id: PROVIDER_ID,
          completion_epoch: 8,
        },
      }),
    /expected_authority/,
  );
  assert.throws(
    () =>
      noritoEncodeInstruction({
        CompleteReplicationOrder: {
          order_id: ORDER_ID,
          provider_id: PROVIDER_ID,
          completion_epoch: 8,
          expected_authority: {
            provider_owner: PROVIDER_OWNER,
            signer_policy: {
              policy_id: POLICY_ID,
              revision: 2,
              predecessor_digest: PREDECESSOR_DIGEST,
              policy_digest: POLICY_DIGEST,
            },
          },
          expected_assignment_revision: 3,
          finalized_anchor: { height: 41, block_hash: BLOCK_HASH },
          relayer_id: "confused-deputy",
        },
      }),
    /unknown field relayer_id/,
  );
});

test("IssueReplicationOrder rejects noncanonical and semantically invalid Norito orders", () => {
  assert.throws(
    () =>
      buildIssueReplicationOrderInstruction({
        orderId: ORDER_ID,
        orderPayload: `${ORDER_PAYLOAD_BASE64}\n`,
        issuedEpoch: 1,
        deadlineEpoch: 2,
      }),
    /exact standard-base64/,
  );
  assert.throws(
    () =>
      buildIssueReplicationOrderInstruction({
        orderId: "ac".repeat(32),
        orderPayload: ORDER_PAYLOAD_BASE64,
        issuedEpoch: 1,
        deadlineEpoch: 2,
      }),
    /must match ReplicationOrderV1.order_id/,
  );
  assert.throws(
    () =>
      buildIssueReplicationOrderInstruction({
        orderId: ORDER_ID,
        orderPayload: Buffer.alloc(
          SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1 + 1,
        ).toString("base64"),
        issuedEpoch: 1,
        deadlineEpoch: 2,
      }),
    /decoded limit/,
  );

  const duplicateProvider = mutatePayload((bytes) => {
    replaceUnique(bytes, Buffer.alloc(32, 0x11), Buffer.alloc(32, 0x10));
  });
  assert.throws(
    () =>
      buildIssueReplicationOrderInstruction({
        orderId: ORDER_ID,
        orderPayload: duplicateProvider,
        issuedEpoch: 1,
        deadlineEpoch: 2,
      }),
    /unique, strictly increasing provider_id/,
  );

  const zeroTarget = mutatePayload((bytes) => {
    replaceUnique(bytes, Buffer.from([0x02, 0x02, 0x00]), Buffer.from([0x02, 0x00, 0x00]));
  });
  assert.throws(
    () =>
      buildIssueReplicationOrderInstruction({
        orderId: ORDER_ID,
        orderPayload: zeroTarget,
        issuedEpoch: 1,
        deadlineEpoch: 2,
      }),
    /target_replicas must be greater than zero/,
  );

  const invalidDeadline = mutatePayload((bytes) => {
    const issued = Buffer.alloc(8);
    issued.writeBigUInt64LE(1_700_000_000n);
    const deadline = Buffer.alloc(8);
    deadline.writeBigUInt64LE(1_700_086_400n);
    replaceUnique(bytes, deadline, issued);
  });
  assert.throws(
    () =>
      buildIssueReplicationOrderInstruction({
        orderId: ORDER_ID,
        orderPayload: invalidDeadline,
        issuedEpoch: 1,
        deadlineEpoch: 2,
      }),
    /deadline_at must be greater than issued_at/,
  );
});
