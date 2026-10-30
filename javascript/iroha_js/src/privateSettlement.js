/** Node confidential-payment sessions backed by the sole native workflow owner. */
import {
  defaultNativeRuntime,
  resolveNativeRuntimeBinding,
} from "./nativeRuntime.js";

const ISSUED = Symbol("native private settlement capability");
const TYPED_ARRAY = Object.getPrototypeOf(Uint8Array.prototype);
const ARRAY_BUFFER = Object.getOwnPropertyDescriptor(TYPED_ARRAY, "buffer").get;
const ARRAY_BYTES = Object.getOwnPropertyDescriptor(TYPED_ARRAY, "byteLength").get;
const ARRAY_KIND = Object.getOwnPropertyDescriptor(TYPED_ARRAY, Symbol.toStringTag).get;
const BUFFER_BYTES = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "byteLength").get;
const BUFFER_RESIZABLE = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "resizable").get;
const COMMANDS = new Map([["prepare", 0], ["prepared", 1], ["run", 2], ["recover", 3]]);
const KINDS = new Set([
  "opened", "staged", "prepared", "not_prepared", "pending", "settled", "aborted",
  "reconciliation_pending_settled", "reconciliation_pending_aborted", "failed",
]);
const LIMITS = [
  "maximumSessions", "maximumOriginalBytes", "maximumArtifactBytes", "maximumPreparedBytes",
];
const OPTIONS = [
  "maxFinalityProofReads", "observationTimeoutMs", "pollIntervalMs",
  "retransmitIdenticalBytes", "retryRejectedStep",
];

function issued(token) {
  if (token !== ISSUED) throw new TypeError("private settlement capabilities are native-issued");
}

function fields(value, allowed, name) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${name} must be an object`);
  }
  const result = Object.create(null);
  for (const key of Reflect.ownKeys(value)) {
    const field = Object.getOwnPropertyDescriptor(value, key);
    if (!allowed.includes(key) || !field || !("value" in field)) {
      throw new TypeError(`${name} contains an unsupported field`);
    }
    result[key] = field.value;
  }
  return result;
}

function integer(value, minimum, maximum, name) {
  if (!Number.isSafeInteger(value) || value < minimum || value > maximum) {
    throw new RangeError(`${name} is outside its native range`);
  }
  return value;
}

function runOptions(options, defaults) {
  const selected = { ...defaults, ...fields(options, OPTIONS, "run options") };
  integer(selected.maxFinalityProofReads, 0, 65535, "maxFinalityProofReads");
  integer(selected.observationTimeoutMs, 0, 0xffffffff, "observationTimeoutMs");
  integer(selected.pollIntervalMs, 1, 0xffffffff, "pollIntervalMs");
  for (const key of ["retransmitIdenticalBytes", "retryRejectedStep"]) {
    if (typeof selected[key] !== "boolean") throw new TypeError(`${key} must be boolean`);
  }
  return OPTIONS.map((key) => selected[key]);
}

function descriptor(bytes, maximum) {
  let buffer;
  let length;
  try {
    if (Reflect.apply(ARRAY_KIND, bytes, []) !== "Uint8Array") throw new TypeError();
    buffer = Reflect.apply(ARRAY_BUFFER, bytes, []);
    length = Reflect.apply(ARRAY_BYTES, bytes, []);
    // This intrinsic rejects an actual SharedArrayBuffer even if `.buffer` is shadowed.
    Reflect.apply(BUFFER_BYTES, buffer, []);
  } catch {
    throw new TypeError("descriptor must be an unshared Uint8Array");
  }
  if (Reflect.apply(BUFFER_RESIZABLE, buffer, []) || length === 0 || length > maximum) {
    throw new RangeError("descriptor is outside its native byte limit");
  }
  // Native admission precedes its retained copy. Do not copy, parse or provision here.
  return bytes;
}

/** Opaque native result. Exported bytes are data, not a portable verification capability. */
export class PrivateSettlementOutcome {
  #native;
  #kind;
  constructor(token, native) {
    issued(token);
    const kind = native.kind();
    if (!KINDS.has(kind)) throw new TypeError("unsupported native private settlement outcome");
    this.#native = native;
    this.#kind = kind;
    Object.freeze(this);
  }
  get kind() { return this.#kind; }
  get failureCode() { return this.#native.failureCode(); }
  /** Redacted observation data; sequence stays an exact decimal string. */
  get progress() {
    const value = this.#native.progress();
    if (value === null) return null;
    for (const key of ["confirmedSteps", "unresolvedSteps", "failedSteps"]) Object.freeze(value[key]);
    return Object.freeze(value);
  }
  /** Bounded canonical Norito data; importing this data cannot create a verified outcome. */
  data() { return this.#native.data(); }
}

/** One accepted native command. Dropping a waiter does not cancel or release custody. */
export class PrivateSettlementTicket {
  #native;
  #result;
  constructor(token, native) {
    issued(token);
    this.#native = native;
    Object.freeze(this);
  }
  get status() { return this.#native.status(); }
  /** Observe native completion without taking its outcome. */
  wait() { return this.#native.wait(); }
  /** Memoize the one native take so concurrent callers receive the same capability. */
  result() {
    this.#result ??= this.wait().then(() => new PrivateSettlementOutcome(ISSUED, this.#native.take()));
    return this.#result;
  }
}

/** A native pending-payment session. Closing drains accepted work; it does not reverse money. */
export class PrivateSettlementOperation {
  #anchor;
  #opening;
  #defaults;
  #descriptorMaximum;
  #closing;
  #closed = false;
  constructor(token, anchor, defaults, descriptorMaximum) {
    issued(token);
    this.#anchor = anchor;
    this.#opening = new PrivateSettlementTicket(ISSUED, anchor);
    this.#defaults = defaults;
    this.#descriptorMaximum = descriptorMaximum;
    Object.freeze(this);
  }
  /** Original opening command, including any redacted native failure. */
  ready() { return this.#opening.result(); }
  /** Accept one command after opening. Busy sessions refuse in the native owner. */
  async start(command, options = {}) {
    if (!COMMANDS.has(command)) throw new TypeError("unsupported private settlement command");
    const args = runOptions(options, this.#defaults);
    if (this.#closed) throw new Error("private settlement operation is closing");
    const opened = await this.ready();
    if (this.#closed) throw new Error("private settlement operation is closing");
    if (opened.kind !== "opened") return this.#opening;
    return new PrivateSettlementTicket(ISSUED, this.#anchor.start(COMMANDS.get(command), ...args));
  }
  /** Explicitly provision native reservation/opening files; recover never calls this. */
  stageFiles(bytes) {
    if (this.#closed) throw new Error("private settlement operation is closing");
    // Call after ready(). Admission/copy is synchronous; there is no borrowed-input await gap.
    return new PrivateSettlementTicket(
      ISSUED, this.#anchor.stageFiles(descriptor(bytes, this.#descriptorMaximum)),
    );
  }
  async prepare(options = {}) { return (await this.start("prepare", options)).result(); }
  async prepared(options = {}) { return (await this.start("prepared", options)).result(); }
  async run(options = {}) { return (await this.start("run", options)).result(); }
  async recover(options = {}) { return (await this.start("recover", options)).result(); }
  /** Await native cleanup before releasing the session's admission. */
  close() {
    this.#closed = true;
    this.#closing ??= this.#anchor.close();
    return this.#closing;
  }
}

/** Bounded host on the installed native runtime, with no JavaScript workflow or retry loop. */
export class PrivateSettlementHost {
  #native;
  #defaults;
  #descriptorMaximum;
  #closing;
  #closed = false;
  constructor(token, native, defaults, descriptorMaximum) {
    issued(token);
    this.#native = native;
    this.#defaults = defaults;
    this.#descriptorMaximum = descriptorMaximum;
    Object.freeze(this);
  }
  /** Admit an exact native files descriptor; secrets and witnesses stay in native file custody. */
  open(bytes) {
    if (this.#closed) throw new Error("private settlement host is closing");
    return new PrivateSettlementOperation(
      ISSUED, this.#native.openFiles(descriptor(bytes, this.#descriptorMaximum)),
      this.#defaults, this.#descriptorMaximum,
    );
  }
  /** Drain all accepted commands and cleanup before destroying the native runtime. */
  shutdown() {
    this.#closed = true;
    this.#closing ??= this.#native.shutdown();
    return this.#closing;
  }
}

/** Create a finite native owner. All four local custody limits are explicit. */
export async function createPrivateSettlementHost(limits) {
  const selected = fields(limits, LIMITS, "host limits");
  const args = LIMITS.map((key, index) => integer(selected[key], 1, index === 0 ? 65535 : 0xffffffff, key));
  // An injected test runtime cannot issue production verified-outcome capabilities.
  const binding = resolveNativeRuntimeBinding(defaultNativeRuntime);
  for (const key of ["privateSettlementCreateHost", "privateSettlementDefaultRunOptions"]) {
    if (typeof binding[key] !== "function") throw new Error("native private settlement support unavailable");
  }
  const maximum = integer(binding.privateSettlementDescriptorMaxBytes, 1, 0xffffffff, "native descriptor limit");
  const defaults = Object.freeze(fields(binding.privateSettlementDefaultRunOptions(), OPTIONS, "native run defaults"));
  runOptions({}, defaults);
  const native = await binding.privateSettlementCreateHost(...args);
  return new PrivateSettlementHost(ISSUED, native, defaults, maximum);
}
