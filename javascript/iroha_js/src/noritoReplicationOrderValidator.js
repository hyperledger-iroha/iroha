import { Buffer } from "buffer";

/** Bind canonical SoraFS replication-order archive validation to shared Norito primitives. */
export function createNoritoReplicationOrderValidator(
  BASE64_ENCODING,
  FIELD_METADATA,
  HEX_ENCODING,
  ISSUE_ORDER_ID_CONTEXT,
  REPLICATION_ORDER_V1_SCHEMA_HASH,
  SORAFS_REPLICATION_ORDER_CHUNKER_HANDLES_V1,
  SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1,
  TEXT_CANONICAL,
  TEXT_EXCEEDS_THE,
  TEXT_ISSUE_REPLICATION_ORDER,
  TEXT_MUST_BE,
  TEXT_MUST_BE_GREATER_THAN_ZERO,
  TEXT_MUST_CONTAIN,
  TEXT_MUST_NOT_BE,
  TEXT_REPLICATION_ORDER_V1,
  UTF8_ENCODING,
  WIRE_FIELD_ORDER_ID,
  decodeByteVecValue,
  decodeCanonicalReplicationId,
  decodeFixedBytesValue,
  decodeNonzeroFixedBytesHex,
  decodeNoritoFrame,
  decodeNoritoVec,
  decodeOptionValue,
  decodeStringValue,
  decodeStructFields,
  decodeU16Value,
  decodeU32Value,
  decodeU64Value,
  decodeU8Value,
  frameNoritoPayload,
  normalizeBytes,
  rejectType,
  withNoritoLengthFlags,
) {
  function decodeReplicationAssignmentValue(payload, context) {
    const fields = decodeStructFields(payload, context, [
      "provider_id",
      "slice_gib",
      "lane",
    ]);
    const providerId = decodeFixedBytesValue(
      fields.provider_id,
      32,
      `${context}.provider_id`,
    );
    if (providerId.every((byte) => byte === 0)) {
      rejectType(`${context}.provider_id${TEXT_MUST_NOT_BE}zero`);
    }
    const sliceGib = decodeU64Value(fields.slice_gib, `${context}.slice_gib`);
    if (sliceGib === "0") {
      rejectType(`${context}.slice_gib${TEXT_MUST_BE_GREATER_THAN_ZERO}`);
    }
    const lane = decodeOptionValue(
      fields.lane,
      decodeStringValue,
      `${context}.lane`,
    );
    if (
      lane !== null &&
      (lane.length === 0 ||
        Buffer.byteLength(lane, UTF8_ENCODING) > 64 ||
        !/^[a-z0-9._-]+$/u.test(lane))
    ) {
      rejectType(`${context}.lane must be a${TEXT_CANONICAL}lane label`);
    }
    return {
      providerIdHex: providerId.toString(HEX_ENCODING),
      sliceGiB: sliceGib,
      lane,
    };
  }

  function decodeReplicationOrderSlaValue(payload, context) {
    const fields = decodeStructFields(payload, context, [
      "ingest_deadline_secs",
      "min_availability_percent_milli",
      "min_por_success_percent_milli",
    ]);
    const ingestDeadlineSecs = decodeU32Value(
      fields.ingest_deadline_secs,
      `${context}.ingest_deadline_secs`,
    );
    const minAvailabilityPercentMilli = decodeU32Value(
      fields.min_availability_percent_milli,
      `${context}.min_availability_percent_milli`,
    );
    const minPorSuccessPercentMilli = decodeU32Value(
      fields.min_por_success_percent_milli,
      `${context}.min_por_success_percent_milli`,
    );
    if (ingestDeadlineSecs === 0) {
      rejectType(`${context}.ingest_deadline_secs${TEXT_MUST_BE_GREATER_THAN_ZERO}`);
    }
    if (
      minAvailabilityPercentMilli === 0 ||
      minAvailabilityPercentMilli > 100_000 ||
      minPorSuccessPercentMilli === 0 ||
      minPorSuccessPercentMilli > 100_000
    ) {
      rejectType(`${context} percentage thresholds${TEXT_MUST_BE}in 1..=100000`);
    }
    return {
      ingestDeadlineSecs,
      minAvailabilityPercentMilli,
      minPorSuccessPercentMilli,
    };
  }

  function decodeReplicationOrderMetadataValue(payload, context) {
    const fields = decodeStructFields(payload, context, ["key", "value"]);
    const key = decodeStringValue(fields.key, `${context}.key`);
    const value = decodeStringValue(fields.value, `${context}.value`);
    if (
      key.length === 0 ||
      key.trim() !== key ||
      Buffer.byteLength(key, UTF8_ENCODING) > 128 ||
      !/^[a-z0-9._-]+$/u.test(key)
    ) {
      rejectType(`${context}.key must be a${TEXT_CANONICAL}metadata key`);
    }
    if (
      value.length === 0 ||
      value.trim() !== value ||
      Buffer.byteLength(value, UTF8_ENCODING) > 4096 ||
      /\p{Cc}/u.test(value)
    ) {
      rejectType(`${context}.value must be${TEXT_CANONICAL}and at most 4096 bytes`);
    }
    return { key, value };
  }

  /**
   * Validate a canonical Norito `ReplicationOrderV1` archive and its optional
   * instruction-level order identifier binding.
   *
   * @param {ArrayBufferView | ArrayBuffer | Buffer} value
   * @param {string | null} [expectedOrderId]
   * @returns {{orderId: string, manifestCidBase64: string, manifestDigestHex: string, chunkingProfile: string, targetReplicas: number, assignments: Array<{providerIdHex: string, sliceGiB: string, lane: string | null}>, providerIds: string[], issuedAt: string, deadlineAt: string, sla: {ingestDeadlineSecs: number, minAvailabilityPercentMilli: number, minPorSuccessPercentMilli: number}, metadata: Array<{key: string, value: string}>}}
   */
  function validateSorafsReplicationOrderPayloadV1(
    value,
    expectedOrderId = null,
  ) {
    const bytes = Buffer.from(normalizeBytes(value));
    if (
      bytes.length === 0 ||
      bytes.length > SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1
    ) {
      rejectType(`ReplicationOrderV1 payload${TEXT_MUST_CONTAIN}1..${SORAFS_REPLICATION_ORDER_MAX_PAYLOAD_BYTES_V1} bytes`);
    }
    const frame = decodeNoritoFrame(
      bytes,
      "ReplicationOrderV1",
      REPLICATION_ORDER_V1_SCHEMA_HASH,
    );
    const canonical = frameNoritoPayload(
      frame.payload,
      REPLICATION_ORDER_V1_SCHEMA_HASH,
      frame.flags,
    );
    if (!canonical.equals(bytes)) {
      rejectType(("ReplicationOrderV1 payload must use" + TEXT_CANONICAL + "unpadded Norito framing"));
    }

    return withNoritoLengthFlags(frame.flags, () => {
      const fields = decodeStructFields(frame.payload, "ReplicationOrderV1", [
        "version",
        WIRE_FIELD_ORDER_ID,
        "manifest_cid",
        "manifest_digest",
        "chunking_profile",
        "target_replicas",
        "assignments",
        "issued_at",
        "deadline_at",
        "sla",
        FIELD_METADATA,
      ]);
      if (decodeU8Value(fields.version, (TEXT_REPLICATION_ORDER_V1 + "version")) !== 1) {
        rejectType((TEXT_REPLICATION_ORDER_V1 + "version" + TEXT_MUST_BE + "1"));
      }
      const orderIdBytes = decodeFixedBytesValue(
        fields.order_id,
        32,
        (TEXT_REPLICATION_ORDER_V1 + "order_id"),
      );
      if (orderIdBytes.every((byte) => byte === 0)) {
        rejectType((TEXT_REPLICATION_ORDER_V1 + "order_id" + TEXT_MUST_NOT_BE + "zero"));
      }
      const orderId = orderIdBytes.toString(HEX_ENCODING);
      if (expectedOrderId !== null) {
        const expected = decodeCanonicalReplicationId(
          expectedOrderId,
          ISSUE_ORDER_ID_CONTEXT,
        );
        if (!expected.equals(orderIdBytes)) {
          rejectType((TEXT_ISSUE_REPLICATION_ORDER + "order_id must match ReplicationOrderV1.order_id"));
        }
      }

      const manifestCid = decodeByteVecValue(
        fields.manifest_cid,
        (TEXT_REPLICATION_ORDER_V1 + "manifest_cid"),
        36,
      );
      if (
        manifestCid.length !== 36 ||
        manifestCid[0] !== 1 ||
        manifestCid[1] !== 0x71 ||
        manifestCid[2] !== 0x1f ||
        manifestCid[3] !== 32 ||
        manifestCid.subarray(4).every((byte) => byte === 0)
      ) {
        rejectType((TEXT_REPLICATION_ORDER_V1 + "manifest_cid must be" + TEXT_CANONICAL + "CIDv1/dag-cbor/BLAKE3-256 bytes"));
      }
      const manifestDigestHex = decodeNonzeroFixedBytesHex(
        fields.manifest_digest,
        (TEXT_REPLICATION_ORDER_V1 + "manifest_digest"),
      );
      const chunkingProfile = decodeStringValue(
        fields.chunking_profile,
        (TEXT_REPLICATION_ORDER_V1 + "chunking_profile"),
      );
      if (!SORAFS_REPLICATION_ORDER_CHUNKER_HANDLES_V1.has(chunkingProfile)) {
        rejectType((TEXT_REPLICATION_ORDER_V1 + "chunking_profile must be a" + TEXT_CANONICAL + "registered handle"));
      }

      const targetReplicas = decodeU16Value(
        fields.target_replicas,
        (TEXT_REPLICATION_ORDER_V1 + "target_replicas"),
      );
      if (targetReplicas === 0) {
        rejectType((TEXT_REPLICATION_ORDER_V1 + "target_replicas" + TEXT_MUST_BE_GREATER_THAN_ZERO));
      }
      const assignments = decodeNoritoVec(
        fields.assignments,
        (entry, index) =>
          decodeReplicationAssignmentValue(
            entry,
            `${TEXT_REPLICATION_ORDER_V1}assignments[${index}]`,
          ),
        (TEXT_REPLICATION_ORDER_V1 + "assignments"),
      );
      if (
        assignments.length === 0 ||
        assignments.length > 1024 ||
        targetReplicas > assignments.length
      ) {
        rejectType(("ReplicationOrderV1 assignments" + TEXT_MUST_CONTAIN + "1..1024 entries and cover target_replicas"));
      }
      for (let index = 1; index < assignments.length; index += 1) {
        if (assignments[index - 1].providerIdHex >= assignments[index].providerIdHex) {
          rejectType("ReplicationOrderV1 assignments must use unique, strictly increasing provider_id values");
        }
      }

      const issuedAt = decodeU64Value(
        fields.issued_at,
        (TEXT_REPLICATION_ORDER_V1 + "issued_at"),
      );
      const deadlineAt = decodeU64Value(
        fields.deadline_at,
        (TEXT_REPLICATION_ORDER_V1 + "deadline_at"),
      );
      if (BigInt(deadlineAt) <= BigInt(issuedAt)) {
        rejectType((TEXT_REPLICATION_ORDER_V1 + "deadline_at" + TEXT_MUST_BE + "greater than issued_at"));
      }
      const sla = decodeReplicationOrderSlaValue(
        fields.sla,
        (TEXT_REPLICATION_ORDER_V1 + "sla"),
      );
      if (BigInt(sla.ingestDeadlineSecs) > BigInt(deadlineAt) - BigInt(issuedAt)) {
        rejectType((TEXT_REPLICATION_ORDER_V1 + "sla.ingest_deadline_secs" + TEXT_EXCEEDS_THE + "order window"));
      }
      const metadata = decodeNoritoVec(
        fields.metadata,
        (entry, index) =>
          decodeReplicationOrderMetadataValue(
            entry,
            `${TEXT_REPLICATION_ORDER_V1}metadata[${index}]`,
          ),
        (TEXT_REPLICATION_ORDER_V1 + "metadata"),
        64,
      );
      const metadataKeys = new Set();
      let metadataBytes = 0;
      for (const entry of metadata) {
        if (metadataKeys.has(entry.key)) {
          rejectType((TEXT_REPLICATION_ORDER_V1 + "metadata contains a duplicate key"));
        }
        metadataKeys.add(entry.key);
        metadataBytes +=
          Buffer.byteLength(entry.key, UTF8_ENCODING) +
          Buffer.byteLength(entry.value, UTF8_ENCODING);
      }
      if (metadataBytes > 64 * 1024) {
        rejectType((TEXT_REPLICATION_ORDER_V1 + "metadata" + TEXT_EXCEEDS_THE + "65536-byte limit"));
      }
      return {
        orderId,
        manifestCidBase64: manifestCid.toString(BASE64_ENCODING),
        manifestDigestHex,
        chunkingProfile,
        targetReplicas,
        assignments,
        providerIds: assignments.map((assignment) => assignment.providerIdHex),
        issuedAt,
        deadlineAt,
        sla,
        metadata,
      };
    });
  }


  return validateSorafsReplicationOrderPayloadV1;
}
