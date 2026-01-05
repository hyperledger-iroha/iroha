import { getNativeBinding } from "./native.js";
import { computeAxtBindingFromNorito } from "./poseidon2.js";

function assertOptionalUnsigned(value, context) {
  if (value === null || value === undefined) {
    return null;
  }
  if (typeof value === "bigint") {
    if (value < 0n) {
      throw new TypeError(`${context} must be non-negative`);
    }
    if (value > Number.MAX_SAFE_INTEGER) {
      throw new TypeError(`${context} exceeds safe integer range`);
    }
    return Number(value);
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value) || value < 0) {
      throw new TypeError(`${context} must be a non-negative finite number`);
    }
    return Math.trunc(value);
  }
  throw new TypeError(`${context} must be a number`);
}

function ensureObject(value, context) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return value;
}

function parseEnvFlag(value) {
  if (value === null || value === undefined) {
    return false;
  }
  const normalized = String(value).trim().toLowerCase();
  return normalized === "1" || normalized === "true" || normalized === "yes";
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

function normalizeStringList(input, context) {
  const items = [];
  let index = 0;
  for (const value of input ?? []) {
    if (typeof value !== "string") {
      throw new TypeError(`${context}[${index}] must be a string`);
    }
    items.push(value);
    index += 1;
  }
  return sortAndDeduplicate(items);
}

function canonicalizeDataspaceIds(input) {
  const dsids = [];
  let index = 0;
  for (const value of input ?? []) {
    dsids.push(assertUnsigned(value, `dsids[${index}]`));
    index += 1;
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
  let index = 0;
  for (const value of input ?? []) {
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
    index += 1;
  }
  return [...touches.entries()]
    .sort(([left], [right]) => left - right)
    .map(([dsid, manifest]) => ({ dsid, read: manifest.read, write: manifest.write }));
}

function canonicalizeTouchFragments(input) {
  const fragments = new Map();
  let index = 0;
  for (const value of input ?? []) {
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
    index += 1;
  }
  return [...fragments.entries()]
    .sort(([left], [right]) => left - right)
    .map(([dsid, manifest]) => ({ dsid, manifest }));
}

function resolveAxtNative() {
  if (parseEnvFlag(process.env.IROHA_JS_DISABLE_NATIVE)) {
    return null;
  }
  const native = getNativeBinding();
  if (!native) {
    return null;
  }
  if (typeof native.axtBuildDescriptor !== "function") {
    return null;
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
 *   next_min_handle_era: number | null,
 *   next_min_sub_nonce: number | null
 * }}
 */
export function normalizeAxtRejectContext(input, context = "axt reject context") {
  const record = ensureObject(input ?? {}, context);
  const reasonValue = record.reason ?? "unknown";
  const reason = typeof reasonValue === "string" ? reasonValue : String(reasonValue);
  const dataspaceRaw = record.dataspace ?? null;
  const laneRaw = record.target_lane ?? null;
  const snapshotRaw = record.snapshot_version ?? null;
  const nextEraRaw = record.next_min_handle_era ?? null;
  const nextSubNonceRaw = record.next_min_sub_nonce ?? null;
  const detailRaw = record.detail ?? "";
  return {
    reason,
    dataspace: assertOptionalUnsigned(dataspaceRaw, `${context}.dataspace`),
    lane: assertOptionalUnsigned(laneRaw, `${context}.lane`),
    snapshot_version: assertOptionalUnsigned(snapshotRaw, `${context}.snapshot_version`),
    detail: detailRaw === null ? "" : String(detailRaw),
    next_min_handle_era: assertOptionalUnsigned(
      nextEraRaw,
      `${context}.next_min_handle_era`,
    ),
    next_min_sub_nonce: assertOptionalUnsigned(
      nextSubNonceRaw,
      `${context}.next_min_sub_nonce`,
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
 *  nextMinHandleEra: number | null,
 *  nextMinSubNonce: number | null,
 *  reason: string,
 *  snapshotVersion: number | null,
 *  detail: string
 * }>} [overrides] Optional override values.
 * @returns {{
 *  dataspace: number | null,
 *  targetLane: number | null,
 *  nextMinHandleEra: number | null,
 *  nextMinSubNonce: number | null,
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
    nextMinHandleEra:
      overrideRecord.nextMinHandleEra !== undefined
        ? assertOptionalUnsigned(
            overrideRecord.nextMinHandleEra,
            "overrides.nextMinHandleEra",
          )
        : normalized.next_min_handle_era,
    nextMinSubNonce:
      overrideRecord.nextMinSubNonce !== undefined
        ? assertOptionalUnsigned(
            overrideRecord.nextMinSubNonce,
            "overrides.nextMinSubNonce",
          )
        : normalized.next_min_sub_nonce,
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
 * Poseidon binding. When the native binding is unavailable the descriptor and
 * manifest fragments are still canonicalised but binding fields are `null`.
 * @param {object} options Descriptor construction options.
 * @param {Iterable<number> | ArrayLike<number>} options.dsids Dataspaces touched by the descriptor.
 * @param {Iterable<{ dsid: number, read?: Iterable<string> | ArrayLike<string>, write?: Iterable<string> | ArrayLike<string> }>} [options.touches]
 * Touch specifications per dataspace.
 * @param {Iterable<{ dsid: number, manifest?: { read?: Iterable<string> | ArrayLike<string>, write?: Iterable<string> | ArrayLike<string> }, read?: Iterable<string> | ArrayLike<string>, write?: Iterable<string> | ArrayLike<string> }>} [options.touchManifest]
 * Optional runtime touch manifest fragments to canonicalise alongside the descriptor.
 * @returns {{
 *  descriptor: { dsids: number[], touches: Array<{ dsid: number, read: string[], write: string[] }> },
 *  descriptorBytes: Buffer | null,
 *  bindingHex: string | null,
 *  binding: Buffer | null,
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
  if (native) {
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

  const descriptorBytes = record.descriptorBytes ?? record.descriptor_bytes ?? null;
  const bindingBuffer = descriptorBytes
    ? computeAxtBindingFromNorito(descriptorBytes)
    : null;

  return {
    descriptor: { dsids, touches },
    descriptorBytes: descriptorBytes ? Buffer.from(descriptorBytes) : null,
    bindingHex: bindingBuffer ? bindingBuffer.toString("hex") : null,
    binding: bindingBuffer,
    touchManifest,
    native: false,
  };
}

/**
 * Compute an AXT binding from Norito-encoded descriptor bytes.
 * @param {Buffer | Uint8Array | ArrayBuffer} descriptorBytes
 * @returns {Buffer}
 */
export function computeAxtBinding(descriptorBytes) {
  return computeAxtBindingFromNorito(descriptorBytes);
}
