import { getNativeBinding } from "./native.js";

function assertOptionalUnsigned(value, context) {
  if (value === null || value === undefined) {
    return null;
  }
  if (typeof value === "bigint") {
    if (value < 0n) {
      throw new TypeError(`${context} must be non-negative`);
    }
    if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new TypeError(`${context} exceeds safe integer range`);
    }
    return Number(value);
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value) || value < 0) {
      throw new TypeError(`${context} must be a non-negative finite number`);
    }
    if (!Number.isInteger(value) || !Number.isSafeInteger(value)) {
      throw new TypeError(`${context} must be a safe integer`);
    }
    return value;
  }
  throw new TypeError(`${context} must be a number`);
}

function ensureObject(value, context) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return value;
}

function assertUnsigned(value, context) {
  const normalized = assertOptionalUnsigned(value, context);
  if (normalized === null) {
    throw new TypeError(`${context} is required`);
  }
  return normalized;
}

function sortAndDeduplicate(strings) {
  const values = [...strings];
  values.sort((a, b) => {
    if (a === b) return 0;
    return a < b ? -1 : 1;
  });
  return values.filter((value, index) => index === 0 || values[index - 1] !== value);
}

const RUST_WHITESPACE_AT_EDGE = /^\p{White_Space}+|\p{White_Space}+$/gu;

function trimRustWhitespace(value) {
  return value.replace(RUST_WHITESPACE_AT_EDGE, "");
}

function containsUnpairedSurrogate(value) {
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = value.charCodeAt(index + 1);
      if (index + 1 >= value.length || next < 0xdc00 || next > 0xdfff) {
        return true;
      }
      index += 1;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) {
      return true;
    }
  }
  return false;
}

function sortAndDeduplicateUtf8(strings) {
  const entries = [...strings].map((value) => ({
    value,
    utf8: Buffer.from(value, "utf8"),
  }));
  entries.sort((left, right) => Buffer.compare(left.utf8, right.utf8));
  return entries
    .filter((entry, index) => index === 0 || entries[index - 1].value !== entry.value)
    .map((entry) => entry.value);
}

function normalizeStringList(input, context) {
  const items = [];
  const values = Array.from(input ?? []);
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (typeof value !== "string") {
      throw new TypeError(`${context}[${index}] must be a string`);
    }
    if (containsUnpairedSurrogate(value)) {
      throw new TypeError(`${context}[${index}] must contain only Unicode scalar values`);
    }
    const normalized = trimRustWhitespace(value);
    if (normalized.length > 0) {
      items.push(normalized);
    }
  }
  return sortAndDeduplicateUtf8(items);
}

function canonicalizeDataspaceIds(input) {
  const dsids = [];
  const values = Array.from(input ?? []);
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    dsids.push(assertUnsigned(value, `dsids[${index}]`));
  }
  if (dsids.length === 0) {
    throw new Error("dsids must not be empty");
  }
  return sortAndDeduplicate(dsids);
}

export function buildTouchManifest(read, write) {
  return {
    read: normalizeStringList(read ?? [], "read"),
    write: normalizeStringList(write ?? [], "write"),
  };
}

function canonicalizeTouches(input) {
  const touches = new Map();
  const values = Array.from(input ?? []);
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    const record = ensureObject(value ?? {}, `touches[${index}]`);
    const dsid = assertUnsigned(
      record.dsid ?? record.dataspace ?? record.dataspaceId,
      `touches[${index}].dsid`,
    );
    if (touches.has(dsid)) {
      throw new Error(`duplicate touch for dataspace ${dsid}`);
    }
    const manifest = buildTouchManifest(
      record.read ?? record.manifest?.read ?? [],
      record.write ?? record.manifest?.write ?? [],
    );
    touches.set(dsid, manifest);
  }
  return [...touches.entries()]
    .sort(([left], [right]) => left - right)
    .map(([dsid, manifest]) => ({ dsid, read: manifest.read, write: manifest.write }));
}

function canonicalizeTouchFragments(input) {
  const fragments = new Map();
  const values = Array.from(input ?? []);
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    const record = ensureObject(value ?? {}, `touchManifest[${index}]`);
    const dsid = assertUnsigned(
      record.dsid ?? record.dataspace ?? record.dataspaceId,
      `touchManifest[${index}].dsid`,
    );
    if (fragments.has(dsid)) {
      throw new Error(`duplicate touch manifest for dataspace ${dsid}`);
    }
    const manifest = record.manifest ?? {};
    fragments.set(
      dsid,
      buildTouchManifest(record.read ?? manifest.read ?? [], record.write ?? manifest.write ?? []),
    );
  }
  return [...fragments.entries()]
    .sort(([left], [right]) => left - right)
    .map(([dsid, manifest]) => ({ dsid, manifest }));
}

function resolveAxtNative() {
  const native = getNativeBinding();
  if (typeof native.axtBuildDescriptor !== "function") {
    throw new Error("Native binding required; missing axtBuildDescriptor.");
  }
  return native;
}

/**
 * Normalize an AXT reject payload (for example, from a Torii rejection) into a
 * predictable shape with snake_case fields.
 * @param {unknown} input Raw reject context.
 * @param {string} [context] Error prefix used in validation messages.
 * @returns {{
 *   reason: string,
 *   dataspace: number | null,
 *   lane: number | null,
 *   snapshot_version: number | null,
 *   detail: string,
 *   active_handle_era: number | null,
 *   next_handle_counter: number | null
 * }}
 */
export function normalizeAxtRejectContext(input, context = "axt reject context") {
  const record = ensureObject(input, context);
  const retiredOrNoncanonicalKeys = [
    "dataspaceId",
    "targetLane",
    "target_lane",
    "snapshotVersion",
    "activeHandleEra",
    "nextHandleCounter",
    "next_min_handle_era",
    "next_min_sub_nonce",
  ];
  for (const key of retiredOrNoncanonicalKeys) {
    if (record[key] !== undefined) {
      throw new TypeError(
        `${context} must use the canonical AXT fields (dataspace, lane, snapshot_version, active_handle_era, next_handle_counter)`,
      );
    }
  }
  const fields = [
    "reason",
    "dataspace",
    "lane",
    "snapshot_version",
    "detail",
    "active_handle_era",
    "next_handle_counter",
  ];
  const knownFields = new Set(fields);
  for (const key of Object.keys(record)) {
    if (!knownFields.has(key)) {
      throw new TypeError(`${context}.${key} is not a canonical AXT field`);
    }
  }
  for (const field of fields) {
    if (!Object.hasOwn(record, field)) {
      throw new TypeError(`${context}.${field} is required`);
    }
  }
  if (typeof record.reason !== "string") {
    throw new TypeError(`${context}.reason must be a string`);
  }
  if (typeof record.detail !== "string") {
    throw new TypeError(`${context}.detail must be a string`);
  }
  return {
    reason: record.reason,
    dataspace: assertOptionalUnsigned(record.dataspace, `${context}.dataspace`),
    lane: assertOptionalUnsigned(record.lane, `${context}.lane`),
    snapshot_version: assertOptionalUnsigned(
      record.snapshot_version,
      `${context}.snapshot_version`,
    ),
    detail: record.detail,
    active_handle_era: assertOptionalUnsigned(
      record.active_handle_era,
      `${context}.active_handle_era`,
    ),
    next_handle_counter: assertOptionalUnsigned(
      record.next_handle_counter,
      `${context}.next_handle_counter`,
    ),
  };
}

/**
 * Build a minimal handle-refresh request envelope using hints from an AXT
 * rejection context. Optional overrides can fill in missing fields (for
 * example, when the status payload omits the lane).
 * @param {unknown} rejectContext Raw reject context object.
 * @param {Partial<{
 *  dataspace: number | null,
 *  targetLane: number | null,
 *  activeHandleEra: number | null,
 *  nextHandleCounter: number | null,
 *  reason: string,
 *  snapshotVersion: number | null,
 *  detail: string
 * }>} [overrides] Optional override values.
 * @returns {{
 *  dataspace: number | null,
 *  targetLane: number | null,
 *  activeHandleEra: number | null,
 *  nextHandleCounter: number | null,
 *  reason: string,
 *  snapshotVersion: number | null,
 *  detail: string
 * }}
 */
export function buildHandleRefreshRequest(rejectContext, overrides = {}) {
  const normalized = normalizeAxtRejectContext(
    rejectContext,
    "handle refresh reject context",
  );
  const overrideRecord = ensureObject(overrides ?? {}, "handle refresh overrides");
  return {
    dataspace:
      overrideRecord.dataspace !== undefined
        ? assertOptionalUnsigned(overrideRecord.dataspace, "overrides.dataspace")
        : normalized.dataspace,
    targetLane:
      overrideRecord.targetLane !== undefined
        ? assertOptionalUnsigned(overrideRecord.targetLane, "overrides.targetLane")
        : normalized.lane,
    activeHandleEra:
      overrideRecord.activeHandleEra !== undefined
        ? assertOptionalUnsigned(
            overrideRecord.activeHandleEra,
            "overrides.activeHandleEra",
          )
        : normalized.active_handle_era,
    nextHandleCounter:
      overrideRecord.nextHandleCounter !== undefined
        ? assertOptionalUnsigned(
            overrideRecord.nextHandleCounter,
            "overrides.nextHandleCounter",
          )
        : normalized.next_handle_counter,
    reason: overrideRecord.reason ?? normalized.reason,
    snapshotVersion:
      overrideRecord.snapshotVersion !== undefined
        ? assertOptionalUnsigned(
            overrideRecord.snapshotVersion,
            "overrides.snapshotVersion",
          )
        : normalized.snapshot_version,
    detail: overrideRecord.detail ?? normalized.detail,
  };
}

/**
 * Build a canonical AXT descriptor and binding from unordered dataspace/touch
 * declarations. Dataspace ids and touch specs are deduplicated and sorted, and
 * the native binding computes the Norito-encoded descriptor bytes plus the
 * Poseidon binding.
 * @param {object} options Descriptor construction options.
 * @param {Iterable<number> | ArrayLike<number>} options.dsids Dataspaces touched by the descriptor.
 * @param {Iterable<{ dsid: number, read?: Iterable<string> | ArrayLike<string>, write?: Iterable<string> | ArrayLike<string> }>} [options.touches]
 * Touch specifications per dataspace.
 * @param {Iterable<{ dsid: number, manifest?: { read?: Iterable<string> | ArrayLike<string>, write?: Iterable<string> | ArrayLike<string> }, read?: Iterable<string> | ArrayLike<string>, write?: Iterable<string> | ArrayLike<string> }>} [options.touchManifest]
 * Optional runtime touch manifest fragments to canonicalise alongside the descriptor.
 * @returns {{
 *  descriptor: { dsids: number[], touches: Array<{ dsid: number, read: string[], write: string[] }> },
 *  descriptorBytes: Buffer,
 *  bindingHex: string | null,
 *  binding: Buffer,
 *  touchManifest: Array<{ dsid: number, manifest: { read: string[], write: string[] } }>,
 *  native: boolean
 * }}
 */
export function buildAxtDescriptor(options = {}) {
  const record = ensureObject(options ?? {}, "axt descriptor options");
  const dsids = canonicalizeDataspaceIds(
    record.dsids ?? record.dataspaces ?? record.dataspaceIds ?? record.dsidList,
  );
  const touches = canonicalizeTouches(record.touches ?? record.touchSpecs ?? []);
  const touchManifest = canonicalizeTouchFragments(
    record.touchManifest ?? record.touchFragments ?? record.touch_manifest ?? [],
  );

  const dsidSet = new Set(dsids);
  for (const touch of touches) {
    if (!dsidSet.has(touch.dsid)) {
      throw new Error(`touch dsid ${touch.dsid} is not present in dsids`);
    }
  }
  for (const fragment of touchManifest) {
    if (!dsidSet.has(fragment.dsid)) {
      throw new Error(`touch manifest dsid ${fragment.dsid} is not present in dsids`);
    }
  }

  const native = resolveAxtNative();
  const artifacts = native.axtBuildDescriptor(
    dsids,
    touches.map((touch) => ({
      dsid: touch.dsid,
      read: touch.read,
      write: touch.write,
    })),
  );
  const descriptorJson = artifacts.descriptor_json ?? artifacts.descriptorJson;
  const manifestJson = artifacts.touch_manifest_json ?? artifacts.touchManifestJson;
  const descriptor = JSON.parse(descriptorJson);
  const manifest = touchManifest.length > 0 ? touchManifest : JSON.parse(manifestJson);
  const bindingHex = artifacts.binding_hex ?? artifacts.bindingHex ?? null;
  return {
    descriptor,
    descriptorBytes: Buffer.from(artifacts.descriptor_bytes ?? artifacts.descriptorBytes),
    bindingHex: typeof bindingHex === "string"
      ? bindingHex.toLowerCase()
      : null,
    binding: Buffer.from(artifacts.binding),
    touchManifest: manifest,
    native: true,
  };
}

/**
 * Compute an AXT binding from Norito-encoded descriptor bytes.
 * @param {Buffer | Uint8Array | ArrayBuffer} descriptorBytes
 * @returns {Buffer}
 */
export function computeAxtBinding(descriptorBytes) {
  const native = getNativeBinding();
  if (typeof native.axtComputeBinding !== "function") {
    throw new Error("Native binding required; missing axtComputeBinding.");
  }
  return Buffer.from(native.axtComputeBinding(descriptorBytes));
}
