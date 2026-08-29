import { Buffer } from "buffer";
import {
  BASE64_ENCODING,
  HEX_ENCODING,
  JS_TYPE_STRING,
} from "./commonLiterals.js";
import { analyzeEntrypointValueTypeV1 } from "./entrypointSchema.js";

/**
 * Bind proof-related codecs to the shared Norito wire primitives without
 * importing the public codec module.
 *
 * @returns {Function[]}
 */
export function createNoritoProofValueCodecs(
  BufferReader,
  LANE_PRIVACY_MERKLE_MAX_DEPTH,
  decodeHashValue,
  decodeNoritoVec,
  decodeOptionValue,
  decodeTupleFields,
  decodeU32Value,
  decodeUnsignedLeb128,
  encodeCompactLength,
  encodeFixedBytesValue,
  encodeHashLiteralBytes,
  encodeNoritoVec,
  encodeOptionValue,
  encodeTupleValue,
  encodeU32Value,
  encodeU8Value,
  isPlainObject,
  normalizeFlexibleBytes,
) {
  const CONFIDENTIAL_MEMO_WIRE_MAGIC_V1 = Buffer.from([
    0x49, 0x52, 0x48, 0x43, 0x4d, 0x31, 0xa5, 0x5a,
  ]);
  const CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1 = 8;
  const CONFIDENTIAL_MEMO_NONCE_BYTES_V1 = 24;
  const CONFIDENTIAL_MEMO_WRAPPED_KEY_BYTES_V1 = 48;
  const CONFIDENTIAL_MEMO_TAG_BYTES_V1 = 16;
  const CONFIDENTIAL_MEMO_MAX_CIPHERTEXT_BYTES_V1 = 64 * 1024;
  const CONFIDENTIAL_MEMO_SUITES_V1 = Object.freeze({
    "ml-kem-768-xchacha20-poly1305-v1": Object.freeze({
      tag: 0,
      encapsulationBytes: 1088,
    }),
    "ml-kem-1024-xchacha20-poly1305-v1": Object.freeze({
      tag: 1,
      encapsulationBytes: 1568,
    }),
  });

  function assertExactObjectKeys(value, expected, context) {
    if (!isPlainObject(value)) {
      throw new TypeError(`${context} must be an object`);
    }
    const expectedSet = new Set(expected);
    for (const key of Object.keys(value)) {
      if (!expectedSet.has(key)) {
        throw new TypeError(`${context} contains unknown field ${key}`);
      }
    }
    for (const key of expected) {
      if (!Object.prototype.hasOwnProperty.call(value, key)) {
        throw new TypeError(`${context}.${key} is required`);
      }
    }
  }

  function requireNonzero(bytes, context) {
    if (bytes.every((byte) => byte === 0)) {
      throw new Error(`${context} must not be all zero`);
    }
    return bytes;
  }

  function confidentialMemoSuite(label, context) {
    if (typeof label !== JS_TYPE_STRING) {
      throw new TypeError(`${context} must be a canonical confidential memo suite label`);
    }
    const suite = CONFIDENTIAL_MEMO_SUITES_V1[label];
    if (suite === undefined) {
      throw new Error(`${context} uses unsupported confidential memo suite ${label}`);
    }
    return suite;
  }

  function confidentialMemoSuiteFromTag(tag, context) {
    for (const [label, suite] of Object.entries(CONFIDENTIAL_MEMO_SUITES_V1)) {
      if (suite.tag === tag) {
        return [label, suite];
      }
    }
    throw new Error(`${context} uses unsupported confidential memo suite tag ${tag}`);
  }

  function encodeMerkleProofValue(value, context) {
    return encodeTupleValue([
      encodeU32Value(value.leaf_index ?? value.leafIndex, `${context}.leaf_index`),
      encodeNoritoVec(value.audit_path ?? value.auditPath ?? [], (entry, index) =>
        encodeOptionValue(
          entry,
          encodeHashLiteralBytes,
          `${context}.audit_path[${index}]`,
        ),
      ),
    ]);
  }

  function decodeMerkleProofValue(payload, context) {
    const fields = decodeTupleFields(payload, context, ["leaf_index", "audit_path"]);
    return {
      leaf_index: decodeU32Value(fields.leaf_index, `${context}.leaf_index`),
      audit_path: decodeNoritoVec(
        fields.audit_path,
        (entry, index) =>
          decodeOptionValue(
            entry,
            decodeHashValue,
            `${context}.audit_path[${index}]`,
          ),
        `${context}.audit_path`,
        LANE_PRIVACY_MERKLE_MAX_DEPTH,
      ),
    };
  }

  function encodeConfidentialMemoEnvelopeV1Value(value, context) {
    assertExactObjectKeys(value, ["slots", "payload_nonce", "ciphertext"], context);
    if (!Array.isArray(value.slots) || value.slots.length !== CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1) {
      throw new RangeError(
        `${context}.slots must contain exactly ${CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1} entries`,
      );
    }

    const encodedSlots = value.slots.map((slot, index) => {
      const slotContext = `${context}.slots[${index}]`;
      assertExactObjectKeys(
        slot,
        ["suite", "encapsulation", "wrap_nonce", "wrapped_memo_key"],
        slotContext,
      );
      const suite = confidentialMemoSuite(slot.suite, `${slotContext}.suite`);
      const encapsulation = requireNonzero(
        Buffer.from(
          normalizeFlexibleBytes(slot.encapsulation, `${slotContext}.encapsulation`),
        ),
        `${slotContext}.encapsulation`,
      );
      if (encapsulation.length !== suite.encapsulationBytes) {
        throw new RangeError(
          `${slotContext}.encapsulation must be exactly ${suite.encapsulationBytes} bytes`,
        );
      }
      const wrapNonce = requireNonzero(
        encodeFixedBytesValue(
          slot.wrap_nonce,
          CONFIDENTIAL_MEMO_NONCE_BYTES_V1,
          `${slotContext}.wrap_nonce`,
        ),
        `${slotContext}.wrap_nonce`,
      );
      const wrappedMemoKey = requireNonzero(
        encodeFixedBytesValue(
          slot.wrapped_memo_key,
          CONFIDENTIAL_MEMO_WRAPPED_KEY_BYTES_V1,
          `${slotContext}.wrapped_memo_key`,
        ),
        `${slotContext}.wrapped_memo_key`,
      );
      return Buffer.concat([
        encodeU8Value(suite.tag, `${slotContext}.suite`),
        encapsulation,
        wrapNonce,
        wrappedMemoKey,
      ]);
    });
    const seenSlots = new Set();
    for (const [index, slot] of encodedSlots.entries()) {
      const identity = slot.toString(HEX_ENCODING);
      if (seenSlots.has(identity)) {
        throw new Error(`${context}.slots[${index}] duplicates an earlier slot`);
      }
      seenSlots.add(identity);
    }

    const payloadNonce = requireNonzero(
      encodeFixedBytesValue(
        value.payload_nonce,
        CONFIDENTIAL_MEMO_NONCE_BYTES_V1,
        `${context}.payload_nonce`,
      ),
      `${context}.payload_nonce`,
    );
    const ciphertext = Buffer.from(
      normalizeFlexibleBytes(value.ciphertext, `${context}.ciphertext`),
    );
    if (
      ciphertext.length < CONFIDENTIAL_MEMO_TAG_BYTES_V1 ||
      ciphertext.length > CONFIDENTIAL_MEMO_MAX_CIPHERTEXT_BYTES_V1
    ) {
      throw new RangeError(
        `${context}.ciphertext must be ${CONFIDENTIAL_MEMO_TAG_BYTES_V1}..${CONFIDENTIAL_MEMO_MAX_CIPHERTEXT_BYTES_V1} bytes`,
      );
    }
    return Buffer.concat([
      CONFIDENTIAL_MEMO_WIRE_MAGIC_V1,
      ...encodedSlots,
      payloadNonce,
      encodeCompactLength(ciphertext.length),
      ciphertext,
    ]);
  }

  function decodeConfidentialMemoEnvelopeV1Value(payload, context) {
    const reader = new BufferReader(payload, context);
    const magic = reader.readBytes(
      CONFIDENTIAL_MEMO_WIRE_MAGIC_V1.length,
      "wire_magic",
    );
    if (!magic.equals(CONFIDENTIAL_MEMO_WIRE_MAGIC_V1)) {
      throw new Error(`${context} has invalid confidential memo V1 wire magic`);
    }
    const slots = [];
    const encodedSlotIdentities = new Set();
    for (let index = 0; index < CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1; index += 1) {
      const slotContext = `${context}.slots[${index}]`;
      const tag = reader.readU8(`${slotContext}.suite`);
      const [suiteLabel, suite] = confidentialMemoSuiteFromTag(tag, `${slotContext}.suite`);
      const encapsulation = requireNonzero(
        reader.readBytes(suite.encapsulationBytes, `${slotContext}.encapsulation`),
        `${slotContext}.encapsulation`,
      );
      const wrapNonce = requireNonzero(
        reader.readBytes(CONFIDENTIAL_MEMO_NONCE_BYTES_V1, `${slotContext}.wrap_nonce`),
        `${slotContext}.wrap_nonce`,
      );
      const wrappedMemoKey = requireNonzero(
        reader.readBytes(
          CONFIDENTIAL_MEMO_WRAPPED_KEY_BYTES_V1,
          `${slotContext}.wrapped_memo_key`,
        ),
        `${slotContext}.wrapped_memo_key`,
      );
      const identity = Buffer.concat([
        Buffer.from([tag]),
        encapsulation,
        wrapNonce,
        wrappedMemoKey,
      ]).toString(HEX_ENCODING);
      if (encodedSlotIdentities.has(identity)) {
        throw new Error(`${slotContext} duplicates an earlier slot`);
      }
      encodedSlotIdentities.add(identity);
      slots.push({
        suite: suiteLabel,
        encapsulation: Buffer.from(encapsulation).toString(BASE64_ENCODING),
        wrap_nonce: Array.from(wrapNonce),
        wrapped_memo_key: Array.from(wrappedMemoKey),
      });
    }
    const payload_nonce = Array.from(
      requireNonzero(
        reader.readBytes(CONFIDENTIAL_MEMO_NONCE_BYTES_V1, "payload_nonce"),
        `${context}.payload_nonce`,
      ),
    );
    const [ciphertextLength, lengthBytes] = decodeUnsignedLeb128(
      payload,
      reader.offset,
      `${context}.ciphertext.length`,
    );
    if (
      ciphertextLength < CONFIDENTIAL_MEMO_TAG_BYTES_V1 ||
      ciphertextLength > CONFIDENTIAL_MEMO_MAX_CIPHERTEXT_BYTES_V1
    ) {
      throw new RangeError(
        `${context}.ciphertext must be ${CONFIDENTIAL_MEMO_TAG_BYTES_V1}..${CONFIDENTIAL_MEMO_MAX_CIPHERTEXT_BYTES_V1} bytes`,
      );
    }
    reader.offset += lengthBytes;
    const ciphertext = reader.readBytes(ciphertextLength, "ciphertext");
    reader.assertEof();
    return {
      slots,
      payload_nonce,
      ciphertext: Buffer.from(ciphertext).toString(BASE64_ENCODING),
    };
  }

  return [
    encodeMerkleProofValue,
    decodeMerkleProofValue,
    encodeConfidentialMemoEnvelopeV1Value,
    decodeConfidentialMemoEnvelopeV1Value,
  ];
}

/**
 * Bind contract-manifest, entrypoint-schema, and related codecs to the shared
 * Norito wire primitives. The one-way dependency keeps initialization acyclic.
 *
 * @returns {Function[]}
 */
export function createNoritoContractCodecs(
  BufferReader,
  assertNonEmptyString,
  assertOnlyObjectKeys,
  decodeAccountIdValue,
  decodeBoolValue,
  decodeConstVecU8Value,
  decodeEventFilterBoxFramePayload,
  decodeHashValue,
  decodeMetadataValue,
  decodeNameValue,
  decodeNoritoVec,
  decodeOptionValue,
  decodePublicKeyValue,
  decodeStringValue,
  decodeStructFields,
  decodeU16Value,
  decodeU32Value,
  decodeU64NumberValue,
  decodeU8Value,
  encodeAccountIdValue,
  encodeBoolValue,
  encodeConstVecU8Value,
  encodeEnumTagValue,
  encodeEventFilterBoxFramePayload,
  encodeHashValue,
  encodeMetadataValue,
  encodeNameValue,
  encodeNoritoStringValue,
  encodeNoritoVec,
  encodeOptionValue,
  encodePublicKeyValue,
  encodeStringValue,
  encodeStructValue,
  encodeU16Value,
  encodeU32Value,
  encodeU64NumberValue,
  encodeU8Value,
  isPlainObject,
  parsePublicKeyLiteral,
  publicKeyLiteralFromParts,
  readNoritoField,
) {
  function assertPlainObjectValue(value, context) {
    if (!isPlainObject(value)) {
      throw new TypeError(`${context} must be an object`);
    }
  }

  function assertArrayValue(value, context) {
    if (!Array.isArray(value)) {
      throw new TypeError(`${context} must be an array`);
    }
  }

  const CONTRACT_MANIFEST_KEYS = Object.freeze([
    "seiyaku_name",
    "code_hash",
    "abi_hash",
    "compiler_fingerprint",
    "features_bitmap",
    "access_set_hints",
    "entrypoints",
    "states",
    "error_codes",
    "kotoba",
    "provenance",
  ]);

  function contractManifestSignatureFields(value, context) {
    assertPlainObjectValue(value, context);
    assertOnlyObjectKeys(value, CONTRACT_MANIFEST_KEYS, context);
    return [
      [encodeOptionValue(value.seiyaku_name, encodeNoritoStringValue, `${context}.seiyaku_name`)],
      [encodeOptionValue(value.code_hash, encodeHashValue, `${context}.code_hash`)],
      [encodeOptionValue(value.abi_hash, encodeHashValue, `${context}.abi_hash`)],
      [encodeOptionValue(value.compiler_fingerprint, encodeNoritoStringValue, `${context}.compiler_fingerprint`)],
      [encodeOptionValue(value.features_bitmap, encodeU64NumberValue, `${context}.features_bitmap`)],
      [encodeOptionValue(value.access_set_hints, encodeAccessSetHintsValue, `${context}.access_set_hints`)],
      [
        encodeOptionValue(
          value.entrypoints ?? null,
          encodeEntrypointDescriptorsValue,
          `${context}.entrypoints`,
        ),
      ],
      [
        encodeOptionValue(
          value.states ?? null,
          encodeStateDescriptorsValue,
          `${context}.states`,
        ),
      ],
      [
        encodeOptionValue(
          value.error_codes ?? null,
          encodeContractErrorCodeDescriptorsValue,
          `${context}.error_codes`,
        ),
      ],
      [
        encodeOptionValue(
          value.kotoba ?? null,
          encodeKotobaTranslationEntriesValue,
          `${context}.kotoba`,
        ),
      ],
    ];
  }

  function encodeContractManifestSignaturePayloadValue(value, context) {
    return encodeStructValue(contractManifestSignatureFields(value, context));
  }

  function encodeContractManifestValue(value, context) {
    return encodeStructValue([
      ...contractManifestSignatureFields(value, context),
      [
        encodeOptionValue(
          value.provenance ?? null,
          encodeManifestProvenanceValue,
          `${context}.provenance`,
        ),
      ],
    ]);
  }

  function decodeContractManifestValue(payload, context) {
    const fields = decodeStructFields(payload, context, [
      "seiyaku_name",
      "code_hash",
      "abi_hash",
      "compiler_fingerprint",
      "features_bitmap",
      "access_set_hints",
      "entrypoints",
      "states",
      "error_codes",
      "kotoba",
      "provenance",
    ]);
    return {
      seiyaku_name: decodeOptionValue(
        fields.seiyaku_name,
        decodeStringValue,
        `${context}.seiyaku_name`,
      ),
      code_hash: decodeOptionValue(
        fields.code_hash,
        decodeHashValue,
        `${context}.code_hash`,
      ),
      abi_hash: decodeOptionValue(
        fields.abi_hash,
        decodeHashValue,
        `${context}.abi_hash`,
      ),
      compiler_fingerprint: decodeOptionValue(
        fields.compiler_fingerprint,
        decodeStringValue,
        `${context}.compiler_fingerprint`,
      ),
      features_bitmap: decodeOptionValue(
        fields.features_bitmap,
        decodeU64NumberValue,
        `${context}.features_bitmap`,
      ),
      access_set_hints: decodeOptionValue(
        fields.access_set_hints,
        decodeAccessSetHintsValue,
        `${context}.access_set_hints`,
      ),
      entrypoints: decodeOptionValue(
        fields.entrypoints,
        decodeEntrypointDescriptorsValue,
        `${context}.entrypoints`,
      ),
      states: decodeOptionValue(
        fields.states,
        decodeStateDescriptorsValue,
        `${context}.states`,
      ),
      error_codes: decodeOptionValue(
        fields.error_codes,
        decodeContractErrorCodeDescriptorsValue,
        `${context}.error_codes`,
      ),
      kotoba: decodeOptionValue(
        fields.kotoba,
        decodeKotobaTranslationEntriesValue,
        `${context}.kotoba`,
      ),
      provenance: decodeOptionValue(
        fields.provenance,
        decodeManifestProvenanceValue,
        `${context}.provenance`,
      ),
    };
  }

  function encodeAccessSetHintsValue(value, context) {
    assertPlainObjectValue(value, context);
    assertOnlyObjectKeys(value, [
      "read_keys",
      "write_keys",
      "dynamic_reads",
      "dynamic_writes",
    ], context);
    return encodeStructValue([
      [encodeNoritoVec(value.read_keys ?? [], (entry, index) =>
        encodeNoritoStringValue(assertNonEmptyString(entry, `${context}.read_keys[${index}]`)),
      )],
      [encodeNoritoVec(value.write_keys ?? [], (entry, index) =>
        encodeNoritoStringValue(assertNonEmptyString(entry, `${context}.write_keys[${index}]`)),
      )],
      [encodeNoritoVec(value.dynamic_reads ?? [], (entry, index) =>
        encodeDynamicAccessHintValue(entry, `${context}.dynamic_reads[${index}]`),
      )],
      [encodeNoritoVec(value.dynamic_writes ?? [], (entry, index) =>
        encodeDynamicAccessHintValue(entry, `${context}.dynamic_writes[${index}]`),
      )],
    ]);
  }

  function decodeAccessSetHintsValue(payload, context) {
    const fields = decodeStructFields(payload, context, [
      "read_keys",
      "write_keys",
      "dynamic_reads",
      "dynamic_writes",
    ]);
    return {
      read_keys: decodeNoritoVec(
        fields.read_keys,
        (entry, index) => decodeStringValue(entry, `${context}.read_keys[${index}]`),
        `${context}.read_keys`,
      ),
      write_keys: decodeNoritoVec(
        fields.write_keys,
        (entry, index) => decodeStringValue(entry, `${context}.write_keys[${index}]`),
        `${context}.write_keys`,
      ),
      dynamic_reads: decodeNoritoVec(
        fields.dynamic_reads,
        (entry, index) =>
          decodeDynamicAccessHintValue(entry, `${context}.dynamic_reads[${index}]`),
        `${context}.dynamic_reads`,
      ),
      dynamic_writes: decodeNoritoVec(
        fields.dynamic_writes,
        (entry, index) =>
          decodeDynamicAccessHintValue(entry, `${context}.dynamic_writes[${index}]`),
        `${context}.dynamic_writes`,
      ),
    };
  }

  function encodeDynamicAccessHintValue(value, context) {
    assertPlainObjectValue(value, context);
    assertOnlyObjectKeys(value, ["base_key", "key_type", "bound_kind", "max_keys"], context);
    return encodeStructValue([
      [encodeNoritoStringValue(assertNonEmptyString(value.base_key, `${context}.base_key`))],
      [encodeNoritoStringValue(assertNonEmptyString(value.key_type, `${context}.key_type`))],
      [encodeNoritoStringValue(assertNonEmptyString(value.bound_kind, `${context}.bound_kind`))],
      [encodeU32Value(value.max_keys, `${context}.max_keys`)],
    ]);
  }

  function decodeDynamicAccessHintValue(payload, context) {
    const fields = decodeStructFields(payload, context, [
      "base_key",
      "key_type",
      "bound_kind",
      "max_keys",
    ]);
    return {
      base_key: decodeStringValue(fields.base_key, `${context}.base_key`),
      key_type: decodeStringValue(fields.key_type, `${context}.key_type`),
      bound_kind: decodeStringValue(fields.bound_kind, `${context}.bound_kind`),
      max_keys: decodeU32Value(fields.max_keys, `${context}.max_keys`),
    };
  }

  function encodeEntrypointDescriptorsValue(value, context) {
    assertArrayValue(value, context);
    return encodeNoritoVec(value, (entry, index) =>
      encodeEntrypointDescriptorValue(entry, `${context}[${index}]`),
    );
  }

  function decodeEntrypointDescriptorsValue(payload, context) {
    return decodeNoritoVec(
      payload,
      (entry, index) => decodeEntrypointDescriptorValue(entry, `${context}[${index}]`),
      context,
    );
  }

  function encodeEntrypointDescriptorValue(value, context) {
    assertPlainObjectValue(value, context);
    assertOnlyObjectKeys(value, [
      "name",
      "kind",
      "params",
      "argument_schema",
      "return_type",
      "return_schema",
      "permission",
      "read_keys",
      "write_keys",
      "access_hints_complete",
      "access_hints_skipped",
      "triggers",
    ], context);
    const triggers = value.triggers ?? [];
    assertArrayValue(triggers, `${context}.triggers`);
    return encodeStructValue([
      [encodeNoritoStringValue(assertNonEmptyString(value.name, `${context}.name`))],
      [encodeEntryPointKindValue(value.kind, `${context}.kind`)],
      [
        encodeNoritoVec(value.params ?? [], (param, index) =>
          encodeEntrypointParamDescriptorValue(param, `${context}.params[${index}]`),
        ),
      ],
      [
        encodeOptionValue(
          value.argument_schema ?? null,
          encodeEntrypointArgumentSchemaValue,
          `${context}.argument_schema`,
        ),
      ],
      [
        encodeOptionValue(
          value.return_type ?? null,
          encodeNoritoStringValue,
          `${context}.return_type`,
        ),
      ],
      [
        encodeOptionValue(
          value.return_schema ?? null,
          encodeEntrypointValueTypeValue,
          `${context}.return_schema`,
        ),
      ],
      [
        encodeOptionValue(
          value.permission ?? null,
          encodeNoritoStringValue,
          `${context}.permission`,
        ),
      ],
      [
        encodeNoritoVec(value.read_keys ?? [], (entry, index) =>
          encodeNoritoStringValue(
            assertNonEmptyString(entry, `${context}.read_keys[${index}]`),
          ),
        ),
      ],
      [
        encodeNoritoVec(value.write_keys ?? [], (entry, index) =>
          encodeNoritoStringValue(
            assertNonEmptyString(entry, `${context}.write_keys[${index}]`),
          ),
        ),
      ],
      [
        encodeOptionValue(
          value.access_hints_complete ?? null,
          encodeBoolValue,
          `${context}.access_hints_complete`,
        ),
      ],
      [
        encodeNoritoVec(
          value.access_hints_skipped ?? [],
          (entry, index) =>
            encodeNoritoStringValue(
              assertNonEmptyString(entry, `${context}.access_hints_skipped[${index}]`),
            ),
        ),
      ],
      [
        encodeNoritoVec(triggers, (entry, index) =>
          encodeManifestTriggerDescriptorValue(entry, `${context}.triggers[${index}]`),
        ),
      ],
    ]);
  }

  function decodeEntrypointDescriptorValue(payload, context) {
    const fields = decodeStructFields(payload, context, [
      "name",
      "kind",
      "params",
      "argument_schema",
      "return_type",
      "return_schema",
      "permission",
      "read_keys",
      "write_keys",
      "access_hints_complete",
      "access_hints_skipped",
      "triggers",
    ]);
    return {
      name: decodeStringValue(fields.name, `${context}.name`),
      kind: decodeEntryPointKindValue(fields.kind, `${context}.kind`),
      params: decodeNoritoVec(
        fields.params,
        (entry, index) => decodeEntrypointParamDescriptorValue(entry, `${context}.params[${index}]`),
        `${context}.params`,
      ),
      argument_schema: decodeOptionValue(
        fields.argument_schema,
        decodeEntrypointArgumentSchemaValue,
        `${context}.argument_schema`,
      ),
      return_type: decodeOptionValue(
        fields.return_type,
        decodeStringValue,
        `${context}.return_type`,
      ),
      return_schema: decodeOptionValue(
        fields.return_schema,
        decodeEntrypointValueTypeValue,
        `${context}.return_schema`,
      ),
      permission: decodeOptionValue(
        fields.permission,
        decodeStringValue,
        `${context}.permission`,
      ),
      read_keys: decodeNoritoVec(
        fields.read_keys,
        (entry, index) => decodeStringValue(entry, `${context}.read_keys[${index}]`),
        `${context}.read_keys`,
      ),
      write_keys: decodeNoritoVec(
        fields.write_keys,
        (entry, index) => decodeStringValue(entry, `${context}.write_keys[${index}]`),
        `${context}.write_keys`,
      ),
      access_hints_complete: decodeOptionValue(
        fields.access_hints_complete,
        decodeBoolValue,
        `${context}.access_hints_complete`,
      ),
      access_hints_skipped: decodeNoritoVec(
        fields.access_hints_skipped,
        (entry, index) =>
          decodeStringValue(entry, `${context}.access_hints_skipped[${index}]`),
        `${context}.access_hints_skipped`,
      ),
      triggers: decodeNoritoVec(
        fields.triggers,
        (entry, index) =>
          decodeManifestTriggerDescriptorValue(entry, `${context}.triggers[${index}]`),
        `${context}.triggers`,
      ),
    };
  }

  function encodeEntryPointKindValue(value, context) {
    const kind = typeof value === JS_TYPE_STRING ? value : value?.kind;
    const normalized = assertNonEmptyString(kind, context).toLowerCase();
    switch (normalized) {
      case "kotoage":
        return encodeEnumTagValue(0);
      case "view":
        return encodeEnumTagValue(1);
      case "hajimari":
        return encodeEnumTagValue(2);
      case "kaizen":
        return encodeEnumTagValue(3);
      default:
        throw new Error(`${context} must be Kotoage, View, Hajimari, or Kaizen`);
    }
  }

  function decodeEntryPointKindValue(payload, context) {
    const reader = new BufferReader(payload, context);
    const tag = reader.readU32LE("tag");
    reader.assertEof();
    switch (tag) {
      case 0:
        return { kind: "Kotoage", value: null };
      case 1:
        return { kind: "View", value: null };
      case 2:
        return { kind: "Hajimari", value: null };
      case 3:
        return { kind: "Kaizen", value: null };
      default:
        throw new Error(`${context} uses unsupported entrypoint kind ${tag}`);
    }
  }

  function encodeEntrypointParamDescriptorValue(value, context) {
    assertPlainObjectValue(value, context);
    assertOnlyObjectKeys(value, ["name", "type_name"], context);
    return encodeStructValue([
      [encodeNoritoStringValue(assertNonEmptyString(value.name, `${context}.name`))],
      [encodeNoritoStringValue(assertNonEmptyString(value.type_name, `${context}.type_name`))],
    ]);
  }

  function decodeEntrypointParamDescriptorValue(payload, context) {
    const fields = decodeStructFields(payload, context, ["name", "type_name"]);
    return {
      name: decodeStringValue(fields.name, `${context}.name`),
      type_name: decodeStringValue(fields.type_name, `${context}.type_name`),
    };
  }

  function encodeEntrypointArgumentSchemaValue(value, context) {
    if (!isPlainObject(value) || !Array.isArray(value.fields)) {
      throw new TypeError(`${context} must contain a fields array`);
    }
    assertOnlyObjectKeys(value, ["fields"], context);
    return encodeStructValue([
      [
        encodeNoritoVec(value.fields, (field, index) =>
          encodeEntrypointArgumentFieldValue(field, `${context}.fields[${index}]`),
        ),
      ],
    ]);
  }

  function decodeEntrypointArgumentSchemaValue(payload, context) {
    const fields = decodeStructFields(payload, context, ["fields"]);
    return {
      fields: decodeNoritoVec(
        fields.fields,
        (field, index) =>
          decodeEntrypointArgumentFieldValue(field, `${context}.fields[${index}]`),
        `${context}.fields`,
      ),
    };
  }

  function encodeEntrypointArgumentFieldValue(value, context) {
    assertPlainObjectValue(value, context);
    assertOnlyObjectKeys(value, ["name", "ty"], context);
    return encodeStructValue([
      [encodeNoritoStringValue(assertNonEmptyString(value.name, `${context}.name`))],
      [encodeEntrypointValueTypeValue(value.ty, `${context}.ty`)],
    ]);
  }

  function decodeEntrypointArgumentFieldValue(payload, context) {
    const fields = decodeStructFields(payload, context, ["name", "ty"]);
    return {
      name: decodeStringValue(fields.name, `${context}.name`),
      ty: decodeEntrypointValueTypeValue(fields.ty, `${context}.ty`),
    };
  }

  function encodeEntrypointValueTypeValue(value, context) {
    if (!isPlainObject(value) || !Array.isArray(value.nodes)) {
      throw new TypeError(`${context} must contain a nodes array`);
    }
    assertOnlyObjectKeys(value, ["nodes"], context);
    analyzeEntrypointValueTypeV1(value, context);
    return encodeStructValue([
      [
        encodeNoritoVec(value.nodes, (node, index) =>
          encodeEntrypointValueTypeNodeValue(node, `${context}.nodes[${index}]`),
        ),
      ],
    ]);
  }

  function decodeEntrypointValueTypeValue(payload, context) {
    const fields = decodeStructFields(payload, context, ["nodes"]);
    const value = {
      nodes: decodeNoritoVec(
        fields.nodes,
        (node, index) =>
          decodeEntrypointValueTypeNodeValue(node, `${context}.nodes[${index}]`),
        `${context}.nodes`,
      ),
    };
    analyzeEntrypointValueTypeV1(value, context);
    return value;
  }

  function taggedEnumParts(value, context) {
    if (!isPlainObject(value)) {
      throw new TypeError(`${context} must be a tagged object`);
    }
    assertOnlyObjectKeys(value, ["kind", "value"], context);
    return {
      kind: assertNonEmptyString(value.kind, `${context}.kind`),
      value: value.value ?? null,
    };
  }

  function encodeEntrypointValueTypeNodeValue(value, context) {
    const tagged = taggedEnumParts(value, context);
    switch (tagged.kind) {
      case "Struct":
        return encodeEnumTagValue(0, () =>
          encodeEntrypointStructTypeNodeValue(tagged.value, `${context}.value`),
        );
      case "Tuple":
        return encodeEnumTagValue(1, () =>
          encodeU16Value(tagged.value, `${context}.value`),
        );
      case "Option":
        requireNullEnumPayload(tagged.value, context);
        return encodeEnumTagValue(2);
      case "Result":
        requireNullEnumPayload(tagged.value, context);
        return encodeEnumTagValue(3);
      case "List":
        return encodeEnumTagValue(4, () =>
          encodeEntrypointListTypeNodeValue(tagged.value, `${context}.value`),
        );
      case "Leaf":
        return encodeEnumTagValue(5, () =>
          encodeEntrypointValueKindValue(tagged.value, `${context}.value`),
        );
      default:
        throw new Error(`${context}.kind uses unsupported value-type node ${tagged.kind}`);
    }
  }

  function decodeEntrypointValueTypeNodeValue(payload, context) {
    const reader = new BufferReader(payload, context);
    const tag = reader.readU32LE("tag");
    switch (tag) {
      case 0:
        return {
          kind: "Struct",
          value: decodeEntrypointStructTypeNodeValue(
            readSingleEnumPayload(reader, context),
            `${context}.value`,
          ),
        };
      case 1:
        return {
          kind: "Tuple",
          value: decodeU16Value(readSingleEnumPayload(reader, context), `${context}.value`),
        };
      case 2:
        reader.assertEof();
        return { kind: "Option", value: null };
      case 3:
        reader.assertEof();
        return { kind: "Result", value: null };
      case 4:
        return {
          kind: "List",
          value: decodeEntrypointListTypeNodeValue(
            readSingleEnumPayload(reader, context),
            `${context}.value`,
          ),
        };
      case 5:
        return {
          kind: "Leaf",
          value: decodeEntrypointValueKindValue(
            readSingleEnumPayload(reader, context),
            `${context}.value`,
          ),
        };
      default:
        throw new Error(`${context} uses unsupported value-type node tag ${tag}`);
    }
  }

  function readSingleEnumPayload(reader, context) {
    const value = readNoritoField(reader, "value");
    reader.assertEof();
    return value;
  }

  function requireNullEnumPayload(value, context) {
    if (value !== null && value !== undefined) {
      throw new TypeError(`${context}.value must be null for a unit variant`);
    }
  }

  function encodeEntrypointStructTypeNodeValue(value, context) {
    if (!isPlainObject(value) || !Array.isArray(value.fields)) {
      throw new TypeError(`${context} must contain a fields array`);
    }
    assertOnlyObjectKeys(value, ["name", "fields"], context);
    return encodeStructValue([
      [encodeNoritoStringValue(assertNonEmptyString(value.name, `${context}.name`))],
      [
        encodeNoritoVec(value.fields, (field, index) =>
          encodeNoritoStringValue(
            assertNonEmptyString(field, `${context}.fields[${index}]`),
          ),
        ),
      ],
    ]);
  }

  function decodeEntrypointStructTypeNodeValue(payload, context) {
    const fields = decodeStructFields(payload, context, ["name", "fields"]);
    return {
      name: decodeStringValue(fields.name, `${context}.name`),
      fields: decodeNoritoVec(
        fields.fields,
        (field, index) => decodeStringValue(field, `${context}.fields[${index}]`),
        `${context}.fields`,
      ),
    };
  }

  function encodeEntrypointListTypeNodeValue(value, context) {
    assertPlainObjectValue(value, context);
    assertOnlyObjectKeys(value, ["capacity"], context);
    return encodeStructValue([
      [encodeU8Value(value.capacity, `${context}.capacity`)],
    ]);
  }

  function decodeEntrypointListTypeNodeValue(payload, context) {
    const fields = decodeStructFields(payload, context, ["capacity"]);
    return {
      capacity: decodeU8Value(fields.capacity, `${context}.capacity`),
    };
  }

  const ENTRYPOINT_VALUE_KIND_NAMES = Object.freeze([
    "Int",
    "Decimal",
    "Quantity",
    "Bool",
    "String",
    "Json",
    "Name",
    "AccountId",
    "AssetDefinitionId",
    "AssetId",
    "DomainId",
    "NftId",
    "DataSpaceId",
    "Blob",
  ]);

  function encodeEntrypointValueKindValue(value, context) {
    const tagged = taggedEnumParts(value, context);
    requireNullEnumPayload(tagged.value, context);
    const tag = ENTRYPOINT_VALUE_KIND_NAMES.indexOf(tagged.kind);
    if (tag < 0) {
      throw new Error(`${context}.kind uses unsupported value kind ${tagged.kind}`);
    }
    return encodeEnumTagValue(tag);
  }

  function decodeEntrypointValueKindValue(payload, context) {
    const reader = new BufferReader(payload, context);
    const tag = reader.readU32LE("tag");
    reader.assertEof();
    const kind = ENTRYPOINT_VALUE_KIND_NAMES[tag];
    if (kind === undefined) {
      throw new Error(`${context} uses unsupported value-kind tag ${tag}`);
    }
    return { kind, value: null };
  }

  function encodeStateDescriptorsValue(value, context) {
    assertArrayValue(value, context);
    return encodeNoritoVec(value, (entry, index) =>
      encodeStateDescriptorValue(entry, `${context}[${index}]`),
    );
  }

  function decodeStateDescriptorsValue(payload, context) {
    return decodeNoritoVec(
      payload,
      (entry, index) => decodeStateDescriptorValue(entry, `${context}[${index}]`),
      context,
    );
  }

  function encodeStateDescriptorValue(value, context) {
    assertPlainObjectValue(value, context);
    assertOnlyObjectKeys(value, ["name", "type_name"], context);
    return encodeStructValue([
      [encodeNoritoStringValue(assertNonEmptyString(value.name, `${context}.name`))],
      [
        encodeNoritoStringValue(
          assertNonEmptyString(value.type_name, `${context}.type_name`),
        ),
      ],
    ]);
  }

  function decodeStateDescriptorValue(payload, context) {
    const fields = decodeStructFields(payload, context, ["name", "type_name"]);
    return {
      name: decodeStringValue(fields.name, `${context}.name`),
      type_name: decodeStringValue(fields.type_name, `${context}.type_name`),
    };
  }

  function encodeContractErrorCodeDescriptorsValue(value, context) {
    assertArrayValue(value, context);
    return encodeNoritoVec(value, (entry, index) =>
      encodeContractErrorCodeDescriptorValue(entry, `${context}[${index}]`),
    );
  }

  function decodeContractErrorCodeDescriptorsValue(payload, context) {
    return decodeNoritoVec(
      payload,
      (entry, index) =>
        decodeContractErrorCodeDescriptorValue(entry, `${context}[${index}]`),
      context,
    );
  }

  function encodeContractErrorCodeDescriptorValue(value, context) {
    assertPlainObjectValue(value, context);
    assertOnlyObjectKeys(value, ["namespace", "name", "code"], context);
    return encodeStructValue([
      [
        encodeNoritoStringValue(
          assertNonEmptyString(value.namespace, `${context}.namespace`),
        ),
      ],
      [encodeNoritoStringValue(assertNonEmptyString(value.name, `${context}.name`))],
      [encodeU32Value(value.code, `${context}.code`)],
    ]);
  }

  function decodeContractErrorCodeDescriptorValue(payload, context) {
    const fields = decodeStructFields(payload, context, ["namespace", "name", "code"]);
    return {
      namespace: decodeStringValue(fields.namespace, `${context}.namespace`),
      name: decodeStringValue(fields.name, `${context}.name`),
      code: decodeU32Value(fields.code, `${context}.code`),
    };
  }

  function encodeKotobaTranslationEntriesValue(value, context) {
    assertArrayValue(value, context);
    return encodeNoritoVec(value, (entry, index) =>
      encodeKotobaTranslationEntryValue(entry, `${context}[${index}]`),
    );
  }

  function decodeKotobaTranslationEntriesValue(payload, context) {
    return decodeNoritoVec(
      payload,
      (entry, index) => decodeKotobaTranslationEntryValue(entry, `${context}[${index}]`),
      context,
    );
  }

  function encodeKotobaTranslationEntryValue(value, context) {
    assertPlainObjectValue(value, context);
    assertOnlyObjectKeys(value, ["msg_id", "translations"], context);
    return encodeStructValue([
      [encodeNoritoStringValue(assertNonEmptyString(value.msg_id, `${context}.msg_id`))],
      [
        encodeNoritoVec(value.translations ?? [], (entry, index) =>
          encodeKotobaTranslationValue(entry, `${context}.translations[${index}]`),
        ),
      ],
    ]);
  }

  function decodeKotobaTranslationEntryValue(payload, context) {
    const fields = decodeStructFields(payload, context, ["msg_id", "translations"]);
    return {
      msg_id: decodeStringValue(fields.msg_id, `${context}.msg_id`),
      translations: decodeNoritoVec(
        fields.translations,
        (entry, index) => decodeKotobaTranslationValue(entry, `${context}.translations[${index}]`),
        `${context}.translations`,
      ),
    };
  }

  function encodeKotobaTranslationValue(value, context) {
    assertPlainObjectValue(value, context);
    assertOnlyObjectKeys(value, ["lang", "text"], context);
    return encodeStructValue([
      [encodeNoritoStringValue(assertNonEmptyString(value.lang, `${context}.lang`))],
      [encodeStringValue(value.text, `${context}.text`)],
    ]);
  }

  function decodeKotobaTranslationValue(payload, context) {
    const fields = decodeStructFields(payload, context, ["lang", "text"]);
    return {
      lang: decodeStringValue(fields.lang, `${context}.lang`),
      text: decodeStringValue(fields.text, `${context}.text`),
    };
  }

  function encodeManifestProvenanceValue(value, context) {
    assertPlainObjectValue(value, context);
    assertOnlyObjectKeys(value, ["signer", "signature"], context);
    const signer = parsePublicKeyLiteral(value.signer, `${context}.signer`);
    const signatureLiteral = assertNonEmptyString(value.signature, `${context}.signature`);
    if (
      signatureLiteral.length % 2 !== 0 ||
      !/^[0-9A-Fa-f]+$/u.test(signatureLiteral)
    ) {
      throw new Error(`${context}.signature must be an even-length hexadecimal string`);
    }
    const signature = Buffer.from(signatureLiteral, HEX_ENCODING);
    validateManifestSignatureBytes(signature, `${context}.signature`);
    return encodeStructValue([
      [encodePublicKeyValue(signer, `${context}.signer`)],
      [encodeConstVecU8Value(signature)],
    ]);
  }

  function decodeManifestProvenanceValue(payload, context) {
    const fields = decodeStructFields(payload, context, ["signer", "signature"]);
    const signer = decodePublicKeyValue(fields.signer, `${context}.signer`);
    const signature = decodeConstVecU8Value(fields.signature, `${context}.signature`);
    validateManifestSignatureBytes(signature, `${context}.signature`);
    return {
      signer: publicKeyLiteralFromParts(
        signer.curve,
        signer.publicKey,
        `${context}.signer`,
      ),
      signature: signature.toString(HEX_ENCODING).toUpperCase(),
    };
  }

  function validateManifestSignatureBytes(signature, context) {
    if (signature.length === 0) {
      throw new Error(`${context} must not be empty`);
    }
    if (signature.every((byte) => byte === 0)) {
      throw new Error(`${context} must not be all zero`);
    }
  }

  function encodeManifestTriggerDescriptorValue(value, context) {
    assertPlainObjectValue(value, context);
    assertOnlyObjectKeys(value, [
      "id",
      "repeats",
      "filter",
      "authority",
      "metadata",
      "callback",
    ], context);
    return encodeStructValue([
      [encodeTriggerIdValue(value.id, `${context}.id`)],
      [encodeTriggerRepeatsValue(value.repeats, `${context}.repeats`)],
      [encodeEventFilterBoxFramePayload(value.filter, `${context}.filter`)],
      [
        encodeOptionValue(
          value.authority ?? null,
          encodeAccountIdValue,
          `${context}.authority`,
        ),
      ],
      [encodeMetadataValue(value.metadata ?? {}, `${context}.metadata`)],
      [encodeTriggerCallbackValue(value.callback, `${context}.callback`)],
    ]);
  }

  function decodeManifestTriggerDescriptorValue(payload, context) {
    const fields = decodeStructFields(payload, context, [
      "id",
      "repeats",
      "filter",
      "authority",
      "metadata",
      "callback",
    ]);
    return {
      id: decodeTriggerIdValue(fields.id, `${context}.id`),
      repeats: decodeTriggerRepeatsValue(fields.repeats, `${context}.repeats`),
      filter: decodeEventFilterBoxFramePayload(fields.filter, `${context}.filter`),
      authority: decodeOptionValue(
        fields.authority,
        decodeAccountIdValue,
        `${context}.authority`,
      ),
      metadata: decodeMetadataValue(fields.metadata, `${context}.metadata`),
      callback: decodeTriggerCallbackValue(fields.callback, `${context}.callback`),
    };
  }

  function encodeTriggerIdValue(value, context) {
    return encodeStructValue([
      [encodeNameValue(assertNonEmptyString(value, context), `${context}.name`)],
    ]);
  }

  function decodeTriggerIdValue(payload, context) {
    const fields = decodeStructFields(payload, context, ["name"]);
    return decodeNameValue(fields.name, `${context}.name`);
  }

  function encodeTriggerRepeatsValue(value, context) {
    if (!isPlainObject(value)) {
      throw new TypeError(
        `${context} must be {Indefinitely:null} or {Exactly:<u32>}`,
      );
    }
    const keys = Object.keys(value);
    if (keys.length !== 1) {
      throw new TypeError(`${context} must contain exactly one repeat variant`);
    }
    if (keys[0] === "Indefinitely") {
      requireNullEnumPayload(value.Indefinitely, context);
      return encodeEnumTagValue(0);
    }
    if (keys[0] === "Exactly") {
      return encodeEnumTagValue(1, () =>
        encodeU32Value(value.Exactly, `${context}.Exactly`),
      );
    }
    throw new Error(`${context} uses unsupported repeat variant ${keys[0]}`);
  }

  function decodeTriggerRepeatsValue(payload, context) {
    const reader = new BufferReader(payload, context);
    const tag = reader.readU32LE("tag");
    if (tag === 0) {
      reader.assertEof();
      return { Indefinitely: null };
    }
    if (tag === 1) {
      return {
        Exactly: decodeU32Value(
          readSingleEnumPayload(reader, context),
          `${context}.Exactly`,
        ),
      };
    }
    throw new Error(`${context} uses unsupported repeat tag ${tag}`);
  }

  function encodeTriggerCallbackValue(value, context) {
    assertPlainObjectValue(value, context);
    assertOnlyObjectKeys(value, ["namespace", "entrypoint"], context);
    return encodeStructValue([
      [
        encodeOptionValue(
          value.namespace ?? null,
          encodeNoritoStringValue,
          `${context}.namespace`,
        ),
      ],
      [
        encodeNoritoStringValue(
          assertNonEmptyString(value.entrypoint, `${context}.entrypoint`),
        ),
      ],
    ]);
  }

  function decodeTriggerCallbackValue(payload, context) {
    const fields = decodeStructFields(payload, context, ["namespace", "entrypoint"]);
    return {
      namespace: decodeOptionValue(
        fields.namespace,
        decodeStringValue,
        `${context}.namespace`,
      ),
      entrypoint: decodeStringValue(fields.entrypoint, `${context}.entrypoint`),
    };
  }


  return [
    encodeContractManifestSignaturePayloadValue,
    encodeContractManifestValue,
    decodeContractManifestValue,
    encodeManifestProvenanceValue,
    decodeManifestProvenanceValue,
  ];
}
