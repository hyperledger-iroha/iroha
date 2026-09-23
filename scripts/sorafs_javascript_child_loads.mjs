// Original-byte Node24 loader observations for the fixed child. Not a sandbox:
// a later hook can delegate and then transform a returned source. The parent's
// trusted original runtime/bootstrap/candidate closure remains indispensable.
import { createHash } from "node:crypto";
import { isBuiltin, registerHooks } from "node:module";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { NativeCacheObservation } from "./sorafs_javascript_native_cache.mjs";
import { OriginalChildInput } from "./sorafs_javascript_child_input.mjs";

const cacheRecheck = NativeCacheObservation.prototype.recheck;
const inputValue = Object.getOwnPropertyDescriptor(OriginalChildInput.prototype, "value").get;
const MODULE_FORMATS = new Set(["module", "commonjs", "json"]);
const SUBJECTS = ["dist/index.js", "dist/public/norito.js", "dist/public/sorafs.js",
  "dist/toriiClient.js", "dist/native.js", "dist/toriiTestHooks.js"];
const MAX_EVENTS = 16384, MAX_TEXT_BYTES = 4 * 1024 * 1024;
function demand(value, message) {
  if (!value) throw new Error(`JavaScript child loads: ${message}`);
}
function field(row, name) {
  const descriptor = Object.getOwnPropertyDescriptor(row, name);
  demand(descriptor && Object.hasOwn(descriptor, "value"), "loader result lacks an own data field");
  return descriptor.value;
}
function canonicalFile(url) {
  demand(typeof url === "string" && url.length <= 16384 && url.startsWith("file:"), "not a bounded file URL");
  const name = fileURLToPath(url);
  demand(pathToFileURL(name).href === url && !name.includes("\0"), "URL alias or noncanonical file name");
  return name;
}

/** One retained observational owner, bound only to the original inherited input.
 * It neither parses npm originals nor confers authority on descriptor claims.
 * Every mandatory subject must have a positive original load; cached resolutions
 * alone do not satisfy that requirement. Unknown URL/load refusals poison it.
 */
export class ChildLoadObservations {
  #input; #allowed = new Map(); #required = new Set(); #preloaded = new Set();
  #loads = new Map(); #resolutions = []; #hook; #active = false; #failed = false; #closed = false;
  #events = 0; #text = 0; #nativeURL; #nativeCache;
  constructor(originalInput) {
    this.#input = inputValue.call(originalInput);
    for (const [root, rows, kind] of [
      [this.#input.installedRoot, this.#input.installed, "installed"],
      [this.#input.coreRoot, this.#input.source, "source"],
      [this.#input.toolsRoot, this.#input.tools, "tool"],
    ]) for (const row of rows) {
      const url = pathToFileURL(join(root, row.path)).href;
      const executable = kind === "source" ? row.path.endsWith(".js")
        : /\.(?:mjs|cjs|js|json)$/u.test(row.path);
      if (executable) {
        demand(!this.#allowed.has(url), "duplicate actual module location");
        this.#allowed.set(url, Object.freeze({ ...row, kind }));
        if (kind === "source") this.#required.add(url);
        if (kind === "tool" && row.path !== "sorafs_javascript_child_entry.mjs") this.#preloaded.add(url);
      }
    }
    for (const name of SUBJECTS) this.#required.add(pathToFileURL(join(
      this.#input.installedRoot, "@iroha/iroha-js", name)).href);
    this.#required.add(pathToFileURL(join(this.#input.toolsRoot, "sorafs_javascript_child_entry.mjs")).href);
    demand([...this.#required].every((url) => this.#allowed.has(url)), "missing mandatory module original");
    try {
      this.#hook = registerHooks({
        resolve: (specifier, context, next) => this.#operation(() => this.#resolve(specifier, context, next)),
        load: (url, context, next) => this.#operation(() => this.#load(url, context, next)),
      });
    } catch (error) { this.#failed = true; this.close(); throw error; }
  }
  #operation(action) {
    if (this.#active) this.#failed = true;
    demand(!this.#failed && !this.#closed, "load owner is refused or closed");
    this.#active = true;
    try {
      demand(++this.#events <= MAX_EVENTS, "loader event bound");
      const result = action(); demand(!this.#failed && !this.#closed, "load owner changed during observation"); return result;
    } catch (error) { this.#failed = true; throw error; }
    finally { this.#active = false; }
  }
  #string(value) {
    demand(typeof value === "string" && value.length <= 16384 && value.isWellFormed(), "loader text bound");
    this.#text += Buffer.byteLength(value); demand(this.#text <= MAX_TEXT_BYTES, "aggregate loader text bound");
    return value;
  }
  #admit(url) {
    this.#string(url);
    if (url.startsWith("node:")) { demand(isBuiltin(url), "unknown builtin"); return "builtin"; }
    const name = canonicalFile(url);
    if (this.#allowed.has(url)) return "file";
    const prefix = this.#input.temporaryRoot + "/";
    const relative = name.startsWith(prefix) ? name.slice(prefix.length) : "";
    const components = relative.split("/");
    demand(components.length === 2 && /^iroha-js-host-[A-Za-z0-9]{6}$/u.test(components[0])
      && components[1] === this.#input.native.sha256 + ".node", "module URL is outside the original closure");
    demand(this.#nativeURL === undefined || this.#nativeURL === url, "multiple native snapshot URLs");
    this.#nativeURL = url;
    return "native";
  }
  #resolve(specifier, context, next) {
    this.#string(specifier);
    const parent = context.parentURL ?? null;
    if (parent !== null) this.#admit(parent);
    const result = next(specifier, context);
    const url = field(result, "url"); this.#admit(url);
    const descriptor = Object.getOwnPropertyDescriptor(result, "format");
    demand(!descriptor || Object.hasOwn(descriptor, "value"), "resolution format accessor");
    const format = descriptor?.value ?? null;
    demand(format === null || MODULE_FORMATS.has(format) || format === "builtin" || format === "addon", "unknown resolution format");
    this.#resolutions.push(Object.freeze({ specifier, parent, url, format }));
    return result;
  }
  #load(url, context, next) {
    const kind = this.#admit(url);
    const result = next(url, context);
    const format = field(result, "format");
    if (kind === "builtin") {
      demand(format === "builtin", "builtin load format differs"); return result;
    }
    demand(!this.#loads.has(url), "same module loaded twice");
    if (kind === "native") {
      demand(format === "addon" && result.source == null, "native load is not an addon");
      // Native source is not supplied by this hook. Actual bytes and ABI belong
      // to the genuine cache/snapshot owners, not these input labels.
      this.#loads.set(url, Object.freeze({ url, kind, format }));
      return result;
    }
    demand(MODULE_FORMATS.has(format), "non-source module format");
    const row = this.#allowed.get(url), source = field(result, "source");
    let bytes;
    if (typeof source === "string") {
      demand(source.length <= row.size && Buffer.byteLength(source) === row.size, "loaded source size differs");
      bytes = source;
    } else {
      demand(ArrayBuffer.isView(source) && source.byteLength === row.size, "loaded source has no exact original bytes");
      bytes = Buffer.from(source.buffer, source.byteOffset, source.byteLength);
    }
    demand(createHash("sha256").update(bytes).digest("hex") === row.sha256, "loaded source bytes differ from original");
    if (row.kind === "source" || row.kind === "tool" || this.#required.has(url))
      demand(format === "module", "fixed source/subject module format differs");
    this.#loads.set(url, Object.freeze({ url, kind: row.kind, format, sha256: row.sha256, size: row.size }));
    return result;
  }
  /** Join the actual native cache; this method does not load or manufacture one. */
  identifyNative(cacheOwner) {
    return this.#operation(() => {
      demand(this.#nativeCache === undefined, "native cache already joined");
      const observed = cacheRecheck.call(cacheOwner);
      const url = pathToFileURL(observed.filename).href;
      demand(this.#admit(url) === "native", "actual native cache is outside the selected snapshot");
      this.#nativeCache = cacheOwner;
    });
  }
  /** After actual imports/tests, require positive loads and original resolution joins. */
  recheck() {
    return this.#operation(() => {
      demand(this.#nativeCache !== undefined
        && pathToFileURL(cacheRecheck.call(this.#nativeCache).filename).href === this.#nativeURL,
        "actual native cache was not retained");
      demand([...this.#required].every((url) => this.#loads.has(url)), "a mandatory source/subject load was not observed");
      demand(this.#resolutions.every((row) => row.url.startsWith("node:")
        || this.#loads.has(row.url) || this.#preloaded.has(row.url) || row.url === this.#nativeURL), "resolution has no original positive load");
      return Object.freeze({ loads: Object.freeze([...this.#loads.values()]),
        resolutions: Object.freeze([...this.#resolutions]) });
    });
  }
  /** Detach once; closure is not proof that stream, sources or process completed. */
  close() {
    if (this.#active) this.#failed = true;
    this.#closed = true;
    const hook = this.#hook; this.#hook = undefined;
    hook?.deregister();
  }
}
