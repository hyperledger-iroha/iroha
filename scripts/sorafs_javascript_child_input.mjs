// Fixed child input observations. Archive/source/candidate authority stays with
// the parent, which passes its original canonical regular file on inherited fd3.
// No SDK, suite, addon, process, archive parser or test callback is loaded here.
import { createHash } from "node:crypto";
import { closeSync, fstatSync, readSync } from "node:fs";
import { basename, isAbsolute, join, normalize } from "node:path";

export const CHILD_TOOL_FILES = Object.freeze([
  "sorafs_javascript_child.mjs", "sorafs_javascript_child_entry.mjs",
  "sorafs_javascript_child_files.mjs", "sorafs_javascript_child_input.mjs",
  "sorafs_javascript_child_loads.mjs", "sorafs_javascript_child_session.mjs",
  "sorafs_javascript_native_cache.mjs", "sorafs_javascript_test_events.mjs",
]);
const CATALOG_SHA256 = "d596804a89108bd60cdafcd9369f800d4e4938605de8435571feff606e37a9b1";
const MAX_INPUT = 8 * 1024 * 1024;
const FULL = ["dev", "ino", "mode", "uid", "gid", "nlink", "size", "mtimeNs", "ctimeNs"];
const CONTRACT = "javascript/iroha_js/test/fixtures/sorafs_native_suite_contract_v1.json";
const ROOT_SUBJECTS = ["dist/index.js", "dist/public/norito.js", "dist/public/sorafs.js",
  "dist/toriiClient.js", "dist/native.js", "dist/toriiTestHooks.js"];

function demand(condition, message) {
  if (!condition) throw new Error(`JavaScript child input: ${message}`);
}
function digest(value) { return createHash("sha256").update(value).digest("hex"); }
function object(value, keys) {
  demand(value !== null && typeof value === "object" && !Array.isArray(value)
    && Object.getPrototypeOf(value) === Object.prototype, "expected a plain input object");
  const actual = Reflect.ownKeys(value);
  demand(actual.length === keys.length && actual.every((key) => keys.includes(key)), "input fields differ");
}
function hex(value, length = 64) {
  demand(typeof value === "string" && value.length === length && /^[0-9a-f]+$/u.test(value)
    && value !== "0".repeat(length), "input digest/commit differs");
}
function absolute(value) {
  demand(typeof value === "string" && value.length <= 4096 && value.isWellFormed()
    && !value.includes("\0") && Buffer.byteLength(value) <= 4096 && isAbsolute(value)
    && normalize(value) === value && value !== "/" && !value.endsWith("/")
    && value.split("/").length <= 65, "input path is not canonical and bounded");
}
function integer(value, minimum, maximum) {
  demand(Number.isSafeInteger(value) && !Object.is(value, -0)
    && value >= minimum && value <= maximum, "input integer bound differs");
}
function relative(value) {
  demand(typeof value === "string" && value.length > 0 && value.length <= 1016
    && value.isWellFormed() && Buffer.byteLength(value) <= 1016 && value.normalize("NFC") === value
    && !value.includes("\\") && !value.includes("\0") && !isAbsolute(value)
    && value.split("/").length <= 64
    && value.split("/").every((part) => part && part !== "." && part !== ".."
      && Buffer.byteLength(part) <= 255), "member path differs");
}
function members(value, count, perFile, aggregate) {
  demand(Array.isArray(value) && value.length > 0 && value.length <= count, "member count bound");
  let total = 0, previous = "";
  const rows = value.map((row) => {
    object(row, ["path", "sha256", "size", "mode"]);
    relative(row.path); hex(row.sha256); integer(row.size, 0, perFile);
    demand(row.path > previous, "members must be uniquely sorted"); previous = row.path;
    demand(row.mode === 0o644 || row.mode === 0o755, "member mode differs");
    total += row.size; demand(total <= aggregate, "aggregate member byte bound");
    return Object.freeze({ ...row });
  });
  return Object.freeze(rows);
}
function canonical(value) {
  const normalized = (item) => Array.isArray(item) ? item.map(normalized)
    : item !== null && typeof item === "object"
      ? Object.fromEntries(Object.keys(item).sort().map((key) => [key, normalized(item[key])])) : item;
  return Buffer.from(JSON.stringify(normalized(value)).replace(/[\u007f-\uffff]/g,
    (char) => "\\u" + char.charCodeAt(0).toString(16).padStart(4, "0")) + "\n", "ascii");
}

/** Decode bounded original bytes, without granting archive/candidate authority. */
export function parseChildInput(raw, expectedSha256) {
  hex(expectedSha256);
  demand(Buffer.isBuffer(raw) && raw.length > 0 && raw.length <= MAX_INPUT
    && digest(raw) === expectedSha256, "original input bytes differ");
  const value = JSON.parse(raw.toString("utf8"));
  object(value, ["schema", "environmentRoot", "temporaryRoot", "native", "catalog", "source", "installed", "tools"]);
  demand(value.schema === "sorafs.javascript.child_input.v1", "input schema differs");
  absolute(value.environmentRoot); absolute(value.temporaryRoot);
  object(value.native, ["originalPath", "sha256", "size", "checksumSha256", "checksumSize",
    "sourceCommit", "nativeSourceTreeSha256", "workspaceSourceTreeSha256"]);
  absolute(value.native.originalPath);
  demand(basename(value.native.originalPath) === "iroha_js_host.node", "normal native loader filename differs");
  for (const key of ["sha256", "checksumSha256", "nativeSourceTreeSha256", "workspaceSourceTreeSha256"]) hex(value.native[key]);
  hex(value.native.sourceCommit, 40); integer(value.native.size, 1, 1024 ** 3);
  integer(value.native.checksumSize, 1, 1024 ** 2);
  demand(typeof value.catalog === "string" && value.catalog.length <= 90 * 1024, "catalog bound");
  const catalogBytes = Buffer.from(value.catalog, "base64");
  demand(catalogBytes.length <= 64 * 1024 && catalogBytes.toString("base64") === value.catalog
    && digest(catalogBytes) === CATALOG_SHA256, "source catalog differs");
  const catalog = JSON.parse(catalogBytes.toString("utf8"));
  const source = members(value.source, 191, 16 * 1024 ** 2, 64 * 1024 ** 2);
  const installed = members(value.installed, 8192, 32 * 1024 ** 2, 192 * 1024 ** 2);
  const tools = members(value.tools, CHILD_TOOL_FILES.length, 16 * 1024 ** 2, 64 * 1024 ** 2);
  const expected = [...catalog.source_files.map((row) => row.path), ...catalog.fixture_files].sort();
  demand(source.length === 191 && expected.length === 191
    && source.every((row, index) => row.path === expected[index] && row.mode === 0o644), "fixed source census differs");
  const sources = new Map(source.map((row) => [row.path, row]));
  demand(catalog.source_files.every((row) => sources.get(row.path).sha256 === row.sha256)
    && sources.has(CONTRACT), "fixed source code or contract differs");
  demand(tools.length === CHILD_TOOL_FILES.length && tools.every((row, index) =>
    row.path === CHILD_TOOL_FILES[index] && row.mode === 0o644), "closed tool extension differs");
  const installedNames = new Set(installed.map((row) => row.path));
  demand(ROOT_SUBJECTS.every((name) => installedNames.has("@iroha/iroha-js/" + name))
    && installedNames.has("@iroha/iroha-js/package.json")
    && installedNames.has(".package-lock.json"), "required installed subjects or generated metadata missing");
  demand(canonical(value).equals(raw), "input is not canonical duplicate-free ASCII JSON");
  return Object.freeze({ schema: value.schema, environmentRoot: value.environmentRoot,
    temporaryRoot: value.temporaryRoot, native: Object.freeze({ ...value.native }),
    source, installed, tools, catalog: value.catalog, inputSha256: expectedSha256,
    coreRoot: join(value.environmentRoot, "qualification/core"),
    toolsRoot: join(value.environmentRoot, "qualification/tools"),
    installedRoot: join(value.environmentRoot, "node_modules") });
}

/** Keep the parent's inherited original file descriptor until final teardown.
 * This observes the exact parent's input file; it does not authenticate its
 * archive/candidate claims or establish physical ownership of other inputs.
 */
export class OriginalChildInput {
  #fd = 3; #stat; #raw; #value; #failed = false; #active = false;
  constructor(expectedSha256) {
    try {
      this.#stat = fstatSync(this.#fd, { bigint: true });
      demand(this.#stat.isFile() && this.#stat.nlink === 1n
        && this.#stat.uid === BigInt(process.getuid()) && (this.#stat.mode & 0o7777n) === 0o600n
        && this.#stat.size > 0n && this.#stat.size <= BigInt(MAX_INPUT), "inherited input is not its bounded private file");
      this.#raw = this.#read(); this.#value = parseChildInput(this.#raw, expectedSha256);
      this.recheck();
    } catch (error) {
      this.#failed = true;
      try { this.close(); } catch (cleanup) { throw new AggregateError([error, cleanup], "child input acquisition and cleanup failed"); }
      throw error;
    }
  }
  #read() {
    demand(this.#fd !== null, "input owner is closed");
    const before = fstatSync(this.#fd, { bigint: true });
    demand(FULL.every((key) => before[key] === this.#stat[key]), "original input descriptor changed");
    const result = Buffer.alloc(Number(this.#stat.size));
    for (let offset = 0; offset < result.length;) {
      const count = readSync(this.#fd, result, offset, Math.min(64 * 1024, result.length - offset), offset);
      demand(count > 0, "original input ended early"); offset += count;
    }
    const extra = Buffer.alloc(1);
    demand(readSync(this.#fd, extra, 0, 1, result.length) === 0
      && FULL.every((key) => fstatSync(this.#fd, { bigint: true })[key] === this.#stat[key]), "original input changed while read");
    return result;
  }
  get value() { demand(!this.#failed && this.#fd !== null, "input owner is refused or closed"); return this.#value; }
  recheck() {
    if (this.#active) this.#failed = true;
    demand(!this.#failed && this.#fd !== null, "input owner is refused or closed");
    this.#active = true;
    try {
      demand(this.#read().equals(this.#raw), "original input bytes changed");
      demand(!this.#failed && this.#fd !== null, "input owner changed during recheck");
      return this.#value;
    } catch (error) { this.#failed = true; throw error; }
    finally { this.#active = false; }
  }
  close() {
    const fd = this.#fd; this.#fd = null;
    if (this.#active) this.#failed = true;
    if (fd !== null) closeSync(fd);
  }
}
