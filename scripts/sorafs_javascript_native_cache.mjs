// Observe the actual CommonJS cache around the installed SDK's normal load.
// No addon is required, reset, snapshotted, or manufactured by this owner.
// Cache state is mutable process state, not native attestation or mapped-memory
// proof. The fixed child must separately retain and verify the named file and
// the original SDK/runtime/native inputs before and after measured execution.
// TODO: connect to that installed child and its retained snapshot file owner.
import { createRequire } from "node:module";
import { isAbsolute, normalize } from "node:path";

const MAX_CACHE_ENTRIES = 4096;
const MAX_EXPORTS = 1024;
const require = createRequire(import.meta.url);

function demand(condition, message) {
  if (!condition) throw new Error(`JavaScript native cache: ${message}`);
}

function data(object, key) {
  const descriptor = Object.getOwnPropertyDescriptor(object, key);
  demand(descriptor && Object.hasOwn(descriptor, "value"), `${String(key)} must be an own data property`);
  return descriptor;
}

function equalDescriptor(left, right) {
  return Object.is(left.value, right.value) && left.writable === right.writable &&
    left.configurable === right.configurable && left.enumerable === right.enumerable;
}

function entries(cache) {
  demand(createRequire(import.meta.url).cache === cache, "cache owner was replaced");
  const keys = Reflect.ownKeys(cache);
  demand(keys.length <= MAX_CACHE_ENTRIES && keys.every((key) =>
    typeof key === "string" && key.length <= 4096),
    "cache inventory exceeds its fixed profile");
  return new Map(keys.map((key) => [key, data(cache, key)]));
}

function nativeEntries(inventory) {
  return [...inventory].filter(([key]) => key.endsWith(".node"));
}

function exportsSnapshot(binding) {
  demand(binding !== null && typeof binding === "object", "exports is not a binding object");
  const keys = Reflect.ownKeys(binding);
  demand(keys.length > 0 && keys.length <= MAX_EXPORTS &&
    keys.every((key) => typeof key === "string" && key.length > 0 && key.length <= 256),
    "exports inventory exceeds its fixed profile");
  const descriptors = keys.map((key) => [key, data(binding, key)]);
  demand(descriptors.some(([, descriptor]) => typeof descriptor.value === "function"),
    "binding has no callable exports");
  return { prototype: Object.getPrototypeOf(binding), descriptors };
}

function sameExports(binding, snapshot) {
  const observed = exportsSnapshot(binding);
  demand(observed.prototype === snapshot.prototype &&
    observed.descriptors.length === snapshot.descriptors.length &&
    observed.descriptors.every(([key, descriptor], i) => key === snapshot.descriptors[i][0] &&
      equalDescriptor(descriptor, snapshot.descriptors[i][1])), "native exports changed");
}

/** Capture before importing or using any installed SDK/native module. */
export class NativeCacheObservation {
  #cache;
  #baseline;
  #loaded;
  #failed = false;
  #active = false;

  constructor() {
    this.#cache = require.cache;
    this.#baseline = entries(this.#cache);
    demand(nativeEntries(this.#baseline).length === 0, "a native module was already loaded");
  }

  #operation(action) {
    if (this.#active) this.#failed = true;
    demand(!this.#failed, "owner was invalidated by an earlier refusal");
    this.#active = true;
    try {
      const result = action();
      demand(!this.#failed, "owner was invalidated during observation");
      return result;
    } catch (error) {
      this.#failed = true;
      throw error;
    } finally {
      this.#active = false;
    }
  }

  #inventory() {
    const inventory = entries(this.#cache);
    for (const [key, descriptor] of this.#baseline) {
      demand(inventory.has(key) && equalDescriptor(descriptor, inventory.get(key)),
        "preexisting module cache entry changed");
    }
    return inventory;
  }

  /** Identify a load that already happened through a normal installed SDK call.
   * The original verified getter must return that same module's exports. It is
   * called only after a loaded native entry exists, so it cannot create evidence.
   */
  identify(getNativeBinding) {
    return this.#operation(() => this.#identify(getNativeBinding));
  }

  #identify(getNativeBinding) {
    const inventory = this.#inventory();
    demand(!this.#loaded, "native module was already identified");
    const candidates = nativeEntries(inventory);
    demand(candidates.length === 1, "expected exactly one already loaded native module");
    const [filename, cacheDescriptor] = candidates[0];
    demand(isAbsolute(filename) && normalize(filename) === filename && !filename.includes("\0"),
      "native cache filename is not absolute and canonical");
    const module = cacheDescriptor.value;
    demand(module !== null && typeof module === "object", "native cache entry is not a module");
    const descriptors = ["id", "filename", "loaded", "exports"].map((key) => [key, data(module, key)]);
    const fields = Object.fromEntries(descriptors.map(([key, descriptor]) => [key, descriptor.value]));
    demand(fields.id === filename && fields.filename === filename && fields.loaded === true,
      "module identity or completed load differs");
    const snapshot = exportsSnapshot(fields.exports);
    demand(typeof getNativeBinding === "function" && getNativeBinding() === fields.exports,
      "installed getter differs from the already loaded binding");
    this.#loaded = { filename, cacheDescriptor, module, descriptors, snapshot };
    return this.#check();
  }

  /** Recheck cache, module and callable identities without loading an addon. */
  recheck() {
    return this.#operation(() => this.#check());
  }

  #check() {
    const inventory = this.#inventory();
    demand(this.#loaded, "native module has not been identified");
    const { filename, cacheDescriptor, module, descriptors, snapshot } = this.#loaded;
    const candidates = nativeEntries(inventory);
    demand(candidates.length === 1 && candidates[0][0] === filename &&
      equalDescriptor(candidates[0][1], cacheDescriptor), "loaded native cache entry changed");
    for (const [key, descriptor] of descriptors) {
      demand(equalDescriptor(data(module, key), descriptor), "loaded native module fields changed");
    }
    sameExports(data(module, "exports").value, snapshot);
    return Object.freeze({ filename, exportNames: Object.freeze(snapshot.descriptors.map(([key]) => key)) });
  }
}
