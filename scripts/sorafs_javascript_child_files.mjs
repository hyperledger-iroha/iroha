// Retain the selected original .node and the normal SDK loader's actual cache-
// identified snapshot on POSIX Node24. No addon is loaded by this module.
// The fixed child supplies independently authenticated original digest/size and
// its owner-only TMPDIR before importing the installed SDK. No environment or
// filesystem mutation is performed. Call close() in finally; close is not a
// successful custody check. A failed operation permanently invalidates the owner.
// Node has no openat: held ancestor descriptors and before/after pathname checks
// detect observed replacement, not a hostile transient rename restored between
// checks. These file observations do not attest already-mapped native memory.
// TODO: connect the fixed installed child after its original source/runtime,
// installed-content, ABI and final test-lifecycle owners are assembled.
import { createHash } from "node:crypto";
import { constants, closeSync, fstatSync, lstatSync, openSync, opendirSync, readSync } from "node:fs";
import { basename, dirname, isAbsolute, join, normalize } from "node:path";
import { NativeCacheObservation } from "./sorafs_javascript_native_cache.mjs";

// Same per-original ceiling as sorafs_sdk_artifact_index.MAX_FILE_BYTES.
const MAX_NATIVE_BYTES = 1024 * 1024 * 1024;
const MAX_PATH_BYTES = 4096;
const MAX_PATH_COMPONENTS = 64;
const MAX_DESCRIPTORS = 132;
const CHUNK_BYTES = 64 * 1024;
const cacheRecheck = NativeCacheObservation.prototype.recheck;
const LINEAGE = ["dev", "ino", "mode", "uid", "gid"];
const FULL = [...LINEAGE, "nlink", "size", "mtimeNs", "ctimeNs"];

function demand(value, message) {
  if (!value) throw new Error(`JavaScript native files: ${message}`);
}
function path(value) {
  demand(typeof value === "string" && value.isWellFormed() && !value.includes("\0")
    && Buffer.byteLength(value) <= MAX_PATH_BYTES && isAbsolute(value)
    && normalize(value) === value && (value === "/" || !value.endsWith("/"))
    && value.split("/").length - 1 <= MAX_PATH_COMPONENTS, "noncanonical or unbounded path");
  return value;
}
function same(left, right, fields = FULL) {
  return fields.every((key) => left[key] === right[key]);
}
function seal(stat) {
  return Object.freeze(Object.fromEntries(FULL.map((key) => [key, stat[key].toString()])));
}
function input(value) {
  demand(value !== null && typeof value === "object"
    && [Object.prototype, null].includes(Object.getPrototypeOf(value)), "expected fixed input object");
  const keys = ["originalPath", "temporaryRoot", "expectedSha256", "expectedSize"];
  const actual = Reflect.ownKeys(value);
  demand(actual.length === keys.length && actual.every((key) => keys.includes(key)), "input fields differ");
  const result = Object.create(null);
  for (const key of keys) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    demand(descriptor && Object.hasOwn(descriptor, "value"), "input accessors are forbidden");
    result[key] = descriptor.value;
  }
  path(result.originalPath); path(result.temporaryRoot);
  demand(result.originalPath.endsWith(".node"), "original must be the selected .node file");
  demand(typeof result.expectedSha256 === "string" && /^[0-9a-f]{64}$/.test(result.expectedSha256)
    && result.expectedSha256 !== "0".repeat(64), "original digest differs");
  demand(Number.isSafeInteger(result.expectedSize) && result.expectedSize > 0
    && result.expectedSize <= MAX_NATIVE_BYTES, "original size bound");
  return result;
}

/** Original file and TMPDIR retained before SDK load, then the exact cache owner.
 * Directory ancestry seals intentionally exclude size/times/link count because
 * other children may be created. The private snapshot directory and both files
 * retain the complete seal, including ctime even when bytes/mtime are restored.
 */
export class NativeSnapshotFiles {
  #input; #directories = new Map(); #files = []; #original; #snapshot; #cache;
  #failed = false; #closed = false; #active = false;

  constructor(options) {
    try {
      demand(process.versions.node.startsWith("24.") && ["darwin", "linux"].includes(process.platform),
        "requires selected POSIX Node24");
      for (const name of ["O_NOFOLLOW", "O_NONBLOCK", "O_DIRECTORY"])
        demand(Number.isInteger(constants[name]) && constants[name] !== 0, `missing ${name}`);
      this.#input = input(options);
      this.#retainAncestors(this.#input.temporaryRoot);
      const temporary = this.#directories.get(this.#input.temporaryRoot).stat;
      demand((temporary.mode & 0o7777n) === 0o700n && temporary.uid === BigInt(process.getuid()),
        "TMPDIR must be owned by the current user with mode0700");
      this.#original = this.#openFile(this.#input.originalPath, false);
      this.#verifyBytes();
    } catch (error) {
      this.#failed = true;
      const cleanup = this.#release();
      if (cleanup.length) throw new AggregateError([error, ...cleanup], "native file acquisition and cleanup failed");
      throw error;
    }
  }

  #count() {
    demand(this.#directories.size + this.#files.length < MAX_DESCRIPTORS, "descriptor bound");
  }
  #flags(directory = false) {
    return constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK
      | (constants.O_CLOEXEC ?? 0) | (directory ? constants.O_DIRECTORY : 0);
  }
  #retainAncestors(directory) {
    const parts = path(directory).split("/").filter(Boolean);
    let current = "/";
    for (let index = 0; index <= parts.length; index++) {
      if (index) current = join(current, parts[index - 1]);
      if (this.#directories.has(current)) continue;
      this.#checkDirectories();
      this.#count();
      const before = lstatSync(current, { bigint: true });
      demand(before.isDirectory() && !before.isSymbolicLink(), "ancestor is not a real directory");
      const fd = openSync(current, this.#flags(true));
      // Retain immediately so every subsequent failure closes this descriptor.
      this.#directories.set(current, { fd, stat: before, strict: false });
      this.#checkDirectories();
    }
  }
  #checkDirectories() {
    for (const [name, row] of this.#directories) {
      const observed = lstatSync(name, { bigint: true });
      const held = fstatSync(row.fd, { bigint: true });
      const fields = row.strict ? FULL : LINEAGE;
      demand(observed.isDirectory() && held.isDirectory()
        && same(row.stat, observed, fields) && same(row.stat, held, fields), "directory lineage changed");
    }
  }
  #openFile(name, snapshot) {
    this.#retainAncestors(dirname(name));
    this.#checkDirectories(); this.#count();
    const before = lstatSync(name, { bigint: true });
    demand(before.isFile() && before.nlink === 1n && before.size === BigInt(this.#input.expectedSize),
      "file must be singly linked, regular, and exactly the original size");
    if (snapshot) demand((before.mode & 0o7777n) === 0o500n
      && before.uid === BigInt(process.getuid()), "snapshot file must have current owner and mode0500");
    else demand((before.mode & 0o22n) === 0n, "original may not be group/other writable");
    const fd = openSync(name, this.#flags());
    const row = { name, fd, stat: before };
    this.#files.push(row);
    this.#checkFile(row); this.#checkDirectories();
    return row;
  }
  #checkFile(row) {
    const observed = lstatSync(row.name, { bigint: true });
    const held = fstatSync(row.fd, { bigint: true });
    demand(observed.isFile() && held.isFile() && same(row.stat, observed) && same(row.stat, held),
      "retained file identity changed");
  }
  #checkSnapshotDirectory() {
    if (!this.#snapshot) return;
    this.#checkDirectories();
    const directory = opendirSync(dirname(this.#snapshot.name), { bufferSize: 1 });
    try {
      const member = directory.readSync();
      demand(member?.name === basename(this.#snapshot.name) && member.isFile()
        && directory.readSync() === null, "snapshot directory must contain only the identified file");
    } finally { directory.closeSync(); }
    this.#checkDirectories();
  }
  #read(row, buffer, length, position) {
    let offset = 0;
    while (offset < length) {
      const count = readSync(row.fd, buffer, offset, length - offset, position + offset);
      demand(count > 0, "original/snapshot ended before its retained size");
      offset += count;
    }
  }
  #verifyBytes() {
    this.#checkDirectories();
    for (const row of this.#files) this.#checkFile(row);
    const original = Buffer.allocUnsafe(CHUNK_BYTES);
    const snapshot = this.#snapshot ? Buffer.allocUnsafe(CHUNK_BYTES) : null;
    const hash = createHash("sha256");
    for (let position = 0; position < this.#input.expectedSize; position += CHUNK_BYTES) {
      const length = Math.min(CHUNK_BYTES, this.#input.expectedSize - position);
      this.#read(this.#original, original, length, position);
      hash.update(original.subarray(0, length));
      if (snapshot) {
        this.#read(this.#snapshot, snapshot, length, position);
        demand(original.subarray(0, length).equals(snapshot.subarray(0, length)),
          "snapshot bytes differ from retained original");
      }
    }
    for (const row of this.#files) {
      demand(readSync(row.fd, original, 0, 1, this.#input.expectedSize) === 0, "file exceeds retained size");
      this.#checkFile(row);
    }
    demand(hash.digest("hex") === this.#input.expectedSha256, "original digest differs");
    this.#checkDirectories();
  }
  #operation(action) {
    if (this.#active) this.#failed = true;
    demand(!this.#closed && !this.#failed, "owner is closed or invalidated");
    this.#active = true;
    try {
      const result = action();
      demand(!this.#failed, "owner was invalidated during observation");
      return result;
    } catch (error) {
      this.#failed = true;
      const cleanup = this.#release();
      if (cleanup.length) throw new AggregateError([error, ...cleanup], "native file refusal and cleanup failed");
      throw error;
    } finally { this.#active = false; }
  }

  /** Consume the genuine cache owner's observation after the SDK's normal load.
   * The fixed child retains both owners; neither a path nor a report can replace
   * the cache owner, whose private-field brand is checked by its original method.
   */
  identify(cacheOwner) {
    return this.#operation(() => {
      demand(!this.#snapshot && !this.#cache, "snapshot was already identified");
      const observation = Reflect.apply(cacheRecheck, cacheOwner, []);
      const filename = path(observation.filename);
      const directory = dirname(filename);
      demand(dirname(directory) === this.#input.temporaryRoot
        && /^iroha-js-host-[A-Za-z0-9]{6}$/.test(basename(directory))
        && basename(filename) === this.#input.expectedSha256 + ".node", "snapshot path differs from loader policy");
      this.#retainAncestors(directory);
      const row = this.#directories.get(directory);
      demand((row.stat.mode & 0o7777n) === 0o700n && row.stat.uid === BigInt(process.getuid()),
        "snapshot directory must have current owner and mode0700");
      row.strict = true;
      this.#cache = cacheOwner;
      this.#snapshot = this.#openFile(filename, true);
      demand(this.#snapshot.stat.dev !== this.#original.stat.dev
        || this.#snapshot.stat.ino !== this.#original.stat.ino, "snapshot aliases the original file");
      return this.#check();
    });
  }
  #check() {
    demand(this.#snapshot && this.#cache, "snapshot has not been identified");
    demand(Reflect.apply(cacheRecheck, this.#cache, []).filename === this.#snapshot.name,
      "cache filename changed");
    this.#checkSnapshotDirectory();
    this.#verifyBytes();
    demand(Reflect.apply(cacheRecheck, this.#cache, []).filename === this.#snapshot.name,
      "cache filename changed during file checks");
    return Object.freeze({ originalPath: this.#original.name, snapshotPath: this.#snapshot.name,
      sha256: this.#input.expectedSha256, size: this.#input.expectedSize,
      original: seal(this.#original.stat), snapshot: seal(this.#snapshot.stat),
      directories: Object.freeze([...this.#directories].map(([name, row]) => Object.freeze({
        path: name, strict: row.strict, seal: seal(row.stat),
      }))) });
  }
  /** Recheck bytes, physical lineage and the same cache without loading native code. */
  recheck() { return this.#operation(() => this.#check()); }

  #release() {
    const errors = [];
    const rows = [...this.#files, ...[...this.#directories.values()].reverse()];
    // Detach ownership before the first close: a close may succeed then report
    // an error, and that descriptor number can already belong to another file.
    // Attempt every original descriptor exactly once, never retry its number.
    this.#files = []; this.#directories.clear(); this.#closed = true;
    for (const row of rows) {
      try { closeSync(row.fd); } catch (error) { errors.push(error); }
    }
    return errors;
  }
  /** Close every retained descriptor. This does not verify or confer success. */
  close() {
    if (this.#active) {
      this.#failed = true;
      throw new Error("JavaScript native files: close during observation");
    }
    if (this.#closed) return;
    const errors = this.#release();
    if (errors.length) throw new AggregateError(errors, "native file cleanup failed");
  }
}
