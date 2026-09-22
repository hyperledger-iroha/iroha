// Actual POSIX Node24 filesystem/descriptor controls with inert .node bytes.
// Cache fixtures are synthetic records in the real process cache: no addon is
// required or executed, and these controls are not SDK/native qualification.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import { createRequire, syncBuiltinESMExports } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { NativeSnapshotFiles } from "../sorafs_javascript_child_files.mjs";
import { NativeCacheObservation } from "../sorafs_javascript_native_cache.mjs";

const require = createRequire(import.meta.url);
const target = resolve(dirname(fileURLToPath(import.meta.url)), "../../target");
fs.mkdirSync(target, { recursive: true });
assert(fs.lstatSync(target).isDirectory(), "test target must be a real directory");
const ROOT = fs.realpathSync(target);
const bytes = Buffer.from("inert native component bytes\n");
const hash = (raw) => createHash("sha256").update(raw).digest("hex");
const refusal = /JavaScript native files:|JavaScript native cache:|ENOENT|ELOOP|ENOTDIR|private member/;

function fixture(t, content = bytes) {
  const directory = fs.mkdtempSync(join(ROOT, "native-file-control-"));
  const temporaryRoot = join(directory, "temporary");
  fs.mkdirSync(temporaryRoot, { mode: 0o700 });
  const originalPath = join(directory, "original.node");
  fs.writeFileSync(originalPath, content, { mode: 0o600 });
  const options = { temporaryRoot, originalPath, expectedSha256: hash(content), expectedSize: content.length };
  const owners = [];
  const cacheOriginals = Object.getOwnPropertyDescriptors(require.cache);
  t.after(() => {
    for (const owner of owners) owner.close();
    for (const key of Reflect.ownKeys(require.cache))
      if (!Object.hasOwn(cacheOriginals, key)) delete require.cache[key];
    Object.defineProperties(require.cache, cacheOriginals);
    fs.rmSync(directory, { force: true, recursive: true });
  });
  const f = { directory, temporaryRoot, originalPath, options,
    open(overrides = {}) {
      const owner = new NativeSnapshotFiles({ ...options, ...overrides });
      owners.push(owner); return owner;
    },
    snapshot() {
      const snapshotDirectory = fs.mkdtempSync(join(temporaryRoot, "iroha-js-host-"));
      fs.chmodSync(snapshotDirectory, 0o700);
      const snapshotPath = join(snapshotDirectory, options.expectedSha256 + ".node");
      fs.writeFileSync(snapshotPath, content, { mode: 0o500 });
      fs.chmodSync(snapshotPath, 0o500);
      return { snapshotDirectory, snapshotPath };
    },
    cache(snapshotPath) {
      const cache = new NativeCacheObservation();
      const binding = { inertOperation() {} };
      const module = { id: snapshotPath, filename: snapshotPath, loaded: true, exports: binding };
      require.cache[snapshotPath] = module;
      cache.identify(() => binding);
      return { cache, module, binding };
    },
  };
  return f;
}
function identified(t, content) {
  const f = fixture(t, content);
  const owner = f.open();
  const snapshot = f.snapshot();
  const loaded = f.cache(snapshot.snapshotPath);
  const observation = owner.identify(loaded.cache);
  return { ...f, ...snapshot, ...loaded, owner, observation };
}
function poisoned(owner, action, pattern = refusal) {
  assert.throws(action, pattern);
  assert.throws(() => owner.recheck(), /closed or invalidated/);
  assert.doesNotThrow(() => owner.close());
}
function instrument(t, replacements) {
  const previous = Object.fromEntries(Object.keys(replacements).map((key) => [key, fs[key]]));
  Object.assign(fs, replacements); syncBuiltinESMExports();
  t.after(() => { Object.assign(fs, previous); syncBuiltinESMExports(); });
  return previous;
}

test("retains actual original, snapshot and directories through repeated recheck and close", (t) => {
  const f = identified(t);
  assert.equal(f.observation.originalPath, f.originalPath);
  assert.equal(f.observation.snapshotPath, f.snapshotPath);
  assert.equal(f.observation.sha256, hash(bytes));
  assert.equal(f.observation.size, bytes.length);
  assert.equal(f.observation.snapshot.mode, String(0o100500));
  assert.equal(f.observation.snapshot.nlink, "1");
  assert(f.observation.directories.every((row) => Object.isFrozen(row) && Object.isFrozen(row.seal)));
  assert.equal(f.observation.directories.filter((row) => row.strict).length, 1);
  assert.deepEqual(f.owner.recheck(), f.observation);
  assert.deepEqual(f.owner.recheck(), f.observation);
  assert.throws(() => { f.observation.snapshot.ino = "0"; }, TypeError);
  f.owner.close(); f.owner.close();
  assert.throws(() => f.owner.recheck(), /closed or invalidated/);
  assert.throws(() => f.owner.identify(f.cache), /closed or invalidated/);
  assert(fs.existsSync(f.snapshotPath), "close must not remove loader-owned files");
});

test("sibling file and directory activity is permitted outside the sealed snapshot directory", (t) => {
  const f = identified(t);
  fs.mkdirSync(join(f.temporaryRoot, "orchestrator"));
  fs.writeFileSync(join(f.directory, "sibling"), "ordinary parent work");
  fs.writeFileSync(join(f.temporaryRoot, "orchestrator", "provider"), "data");
  assert.deepEqual(f.owner.recheck(), f.observation);
  fs.rmSync(join(f.temporaryRoot, "orchestrator"), { recursive: true });
  assert.deepEqual(f.owner.recheck(), f.observation);
});
for (const which of ["original", "snapshot"]) {
  test(`${which} ctime catches rewritten bytes even after exact mtime and modes are restored`, (t) => {
    const f = fixture(t);
    fs.utimesSync(f.originalPath, 1_250_000_000, 1_250_000_000);
    const owner = f.open(); const snapshot = f.snapshot();
    fs.utimesSync(snapshot.snapshotPath, 1_250_000_000, 1_250_000_000);
    const { cache } = f.cache(snapshot.snapshotPath); owner.identify(cache);
    const name = which === "original" ? f.originalPath : snapshot.snapshotPath;
    const before = fs.statSync(name, { bigint: true });
    fs.chmodSync(name, 0o600); fs.writeFileSync(name, Buffer.alloc(bytes.length));
    fs.writeFileSync(name, bytes); fs.chmodSync(name, Number(before.mode & 0o7777n));
    fs.utimesSync(name, 1_250_000_000, 1_250_000_000);
    const after = fs.statSync(name, { bigint: true });
    for (const key of ["dev", "ino", "mode", "uid", "gid", "size", "nlink", "mtimeNs"])
      assert.equal(after[key], before[key], key);
    assert.notEqual(after.ctimeNs, before.ctimeNs);
    assert.deepEqual(fs.readFileSync(name), bytes);
    poisoned(owner, () => owner.recheck());
  });
}

for (const field of ["originalPath", "temporaryRoot"]) {
  for (const form of ["relative", "dot", "trailing", "nul", "surrogate", "depth", "bytes"]) {
    test(`refuses ${field} ${form} path before acquisition`, (t) => {
      const f = fixture(t);
      const value = {
        relative: "relative.node", dot: f.options[field] + "/../name.node",
        trailing: f.options[field] + "/", nul: f.options[field] + "\0",
        surrogate: "/\ud800.node", depth: "/" + "a/".repeat(65) + "x.node",
        bytes: "/" + "x".repeat(4096) + ".node",
      }[form];
      assert.throws(() => f.open({ [field]: value }), /noncanonical or unbounded path/);
    });
  }
}
for (const size of [0, -1, -0, true, 1.5, NaN, Infinity, 1024 ** 3 + 1]) {
  test(`refuses invalid original size ${String(size)}:${typeof size}`, (t) => {
    const f = fixture(t);
    assert.throws(() => f.open({ expectedSize: size }), /original size bound/);
  });
}
for (const digest of ["0".repeat(64), "A".repeat(64), "1".repeat(63), "1".repeat(65), null]) {
  test(`refuses malformed original digest ${String(digest)}`, (t) => {
    const f = fixture(t);
    assert.throws(() => f.open({ expectedSha256: digest }), /original digest differs/);
  });
}
test("closed fixed input fields and accessors cannot execute during acquisition", (t) => {
  const f = fixture(t);
  assert.throws(() => new NativeSnapshotFiles({ ...f.options, extra: true }), /input fields differ/);
  let called = false;
  const options = { ...f.options };
  Object.defineProperty(options, "originalPath", { get() { called = true; return f.originalPath; } });
  assert.throws(() => new NativeSnapshotFiles(options), /accessors/);
  assert.equal(called, false);
});

for (const which of ["original", "snapshot"]) {
  for (const mutation of ["replace", "unlink", "symlink", "chmod", "hardlink", "bytes", "restored_bytes_mtime"]) {
    test(`refuses ${which} ${mutation} and poisons retained owner`, (t) => {
      const f = identified(t);
      const name = which === "original" ? f.originalPath : f.snapshotPath;
      const stat = fs.statSync(name);
      if (mutation === "replace") {
        fs.renameSync(name, name + ".old");
        fs.writeFileSync(name, bytes, { mode: Number(stat.mode & 0o777) });
      } else if (mutation === "unlink") fs.unlinkSync(name);
      else if (mutation === "symlink") {
        fs.renameSync(name, name + ".old"); fs.symlinkSync(name + ".old", name);
      } else if (mutation === "chmod") fs.chmodSync(name, 0o777);
      else if (mutation === "hardlink") fs.linkSync(name, join(f.directory, "linked"));
      else {
        fs.chmodSync(name, 0o600);
        fs.writeFileSync(name, Buffer.alloc(bytes.length, 0x61));
        if (mutation === "restored_bytes_mtime") {
          fs.writeFileSync(name, bytes); fs.chmodSync(name, stat.mode & 0o777);
          fs.utimesSync(name, stat.atime, stat.mtime);
          assert.deepEqual(fs.readFileSync(name), bytes);
        }
      }
      poisoned(f.owner, () => f.owner.recheck());
    });
  }
}

for (const which of ["temporary", "snapshot"]) {
  for (const mutation of ["replace", "symlink", "permissions"]) {
    test(`rejects retained ${which} directory ${mutation}`, (t) => {
      const f = identified(t);
      const name = which === "temporary" ? f.temporaryRoot : f.snapshotDirectory;
      if (mutation === "permissions") fs.chmodSync(name, 0o755);
      else {
        fs.renameSync(name, name + ".old");
        if (mutation === "symlink") fs.symlinkSync(name + ".old", name);
        else {
          fs.cpSync(name + ".old", name, { recursive: true }); fs.chmodSync(name, 0o700);
        }
      }
      poisoned(f.owner, () => f.owner.recheck());
    });
  }
}
test("replaced original ancestor is rejected even when all old descriptors still read unchanged bytes", (t) => {
  const f = identified(t);
  const moved = f.directory + ".old";
  fs.renameSync(f.directory, moved);
  t.after(() => fs.rmSync(moved, { recursive: true, force: true }));
  fs.cpSync(moved, f.directory, { recursive: true });
  assert.deepEqual(fs.readFileSync(f.originalPath), bytes);
  assert.deepEqual(fs.readFileSync(f.snapshotPath), bytes);
  poisoned(f.owner, () => f.owner.recheck());
});

for (const mutation of ["path", "directory_name", "nested", "filename", "directory_mode", "file_mode", "size", "bytes", "extra"]) {
  test(`cache-identified snapshot must satisfy actual loader ${mutation} policy`, (t) => {
    const f = fixture(t); const owner = f.open();
    let { snapshotDirectory, snapshotPath } = f.snapshot();
    if (["path", "directory_name", "nested"].includes(mutation)) {
      let destination = join(f.directory, "outside");
      if (mutation === "directory_name") destination = join(f.temporaryRoot, "foreign-name");
      if (mutation === "nested") { fs.mkdirSync(join(f.temporaryRoot, "nested")); destination = join(f.temporaryRoot, "nested", "iroha-js-host-abcdef"); }
      fs.renameSync(snapshotDirectory, destination); snapshotDirectory = destination;
      snapshotPath = join(destination, f.options.expectedSha256 + ".node");
    } else if (mutation === "filename") {
      const changed = join(snapshotDirectory, "f".repeat(64) + ".node"); fs.renameSync(snapshotPath, changed); snapshotPath = changed;
    } else if (mutation === "directory_mode") fs.chmodSync(snapshotDirectory, 0o755);
    else if (mutation === "file_mode") fs.chmodSync(snapshotPath, 0o600);
    else if (mutation === "extra") fs.writeFileSync(join(snapshotDirectory, "extra"), "unowned");
    else {
      fs.chmodSync(snapshotPath, 0o600);
      fs.writeFileSync(snapshotPath, mutation === "size" ? Buffer.alloc(1) : Buffer.alloc(bytes.length));
      fs.chmodSync(snapshotPath, 0o500);
    }
    const { cache } = f.cache(snapshotPath);
    poisoned(owner, () => owner.identify(cache));
  });
}

test("unidentified, fabricated and overridden cache observations confer no path authority", (t) => {
  const f = fixture(t); const owner = f.open();
  let called = false;
  const forged = { recheck() { called = true; return { filename: f.originalPath }; } };
  poisoned(owner, () => owner.identify(forged), /Receiver must be an instance of class NativeCacheObservation/);
  assert.equal(called, false);
  const pending = f.open();
  poisoned(pending, () => pending.identify(new NativeCacheObservation()));
  const actual = f.open(); const snapshot = f.snapshot(); const { cache } = f.cache(snapshot.snapshotPath);
  cache.recheck = () => { throw new Error("replacement instance method called"); };
  assert.equal(actual.identify(cache).snapshotPath, snapshot.snapshotPath);
});
test("changed actual native cache invalidates the file owner", (t) => {
  const f = identified(t);
  f.module.exports = { inertOperation() {} };
  poisoned(f.owner, () => f.owner.recheck());
});
test("recheck before identification and repeated identification permanently refuse", (t) => {
  const f = fixture(t); const pending = f.open();
  poisoned(pending, () => pending.recheck());
  const owner = f.open(); const snapshot = f.snapshot(); const { cache } = f.cache(snapshot.snapshotPath);
  owner.identify(cache);
  poisoned(owner, () => owner.identify(cache));
});

test("wrong original digest/size and unsafe original or TMPDIR modes fail acquisition", (t) => {
  const f = fixture(t);
  assert.throws(() => f.open({ expectedSha256: "1".repeat(64) }), /original digest differs/);
  assert.throws(() => f.open({ expectedSize: bytes.length + 1 }), /exactly the original size/);
  fs.chmodSync(f.originalPath, 0o666);
  assert.throws(() => f.open(), /group\/other writable/);
  fs.chmodSync(f.originalPath, 0o600); fs.chmodSync(f.temporaryRoot, 0o755);
  assert.throws(() => f.open(), /mode0700/);
});
test("original symlink and multiple links are rejected before file reads", (t) => {
  const f = fixture(t); const linked = join(f.directory, "linked.node");
  fs.symlinkSync(f.originalPath, linked);
  assert.throws(() => f.open({ originalPath: linked }), /singly linked/);
  fs.unlinkSync(linked); fs.linkSync(f.originalPath, linked);
  assert.throws(() => f.open(), /singly linked/);
});

test("multi-chunk and exact chunk-boundary bytes stream with at most64KiB reads", (t) => {
  const source = Buffer.alloc(64 * 1024 * 3 + 17, 0x5a);
  const read = fs.readSync; let calls = 0; let maximum = 0;
  instrument(t, { readSync(fd, buffer, offset, length, position) {
    calls++; maximum = Math.max(maximum, length);
    return read(fd, buffer, offset, Math.min(length, 7919), position);
  } });
  const f = identified(t, source);
  assert.deepEqual(f.owner.recheck(), f.observation);
  assert(calls > 20); assert.equal(maximum, 64 * 1024);
});
test("an exact64KiB file retains its EOF boundary", (t) => {
  const f = identified(t, Buffer.alloc(64 * 1024, 0x5b));
  assert.equal(f.owner.recheck().size, 64 * 1024);
});
test("1GiB boundary reaches stat admission but over-bound refuses before any open", (t) => {
  const f = fixture(t); let opens = 0; const open = fs.openSync;
  instrument(t, { openSync(...args) { opens++; return open(...args); } });
  assert.throws(() => f.open({ expectedSize: 1024 ** 3 + 1 }), /original size bound/);
  assert.equal(opens, 0);
  assert.throws(() => f.open({ expectedSize: 1024 ** 3 }), /exactly the original size/);
  assert(opens > 0);
});

test("leaf symlink substituted after lstat is refused with no-follow open", (t) => {
  const f = fixture(t); const open = fs.openSync; let switched = false;
  instrument(t, { openSync(name, flags, ...rest) {
    if (name === f.originalPath && !switched) {
      switched = true; fs.renameSync(name, name + ".old"); fs.symlinkSync(name + ".old", name);
    }
    return open(name, flags, ...rest);
  } });
  assert.throws(() => f.open(), refusal); assert.equal(switched, true);
});
test("actual FIFO substituted after lstat cannot block the nonblocking leaf open", (t) => {
  const f = fixture(t); const open = fs.openSync; let switched = false;
  instrument(t, { openSync(name, flags, ...rest) {
    if (name === f.originalPath && !switched) {
      switched = true; fs.unlinkSync(name);
      const result = spawnSync("/usr/bin/mkfifo", [name], { encoding: "utf8", timeout: 5000 });
      assert.equal(result.status, 0, result.stderr);
      // Before the real FIFO open, prove the bounded owner kept its required flag.
      assert.notEqual(flags & fs.constants.O_NONBLOCK, 0);
      assert.notEqual(flags & fs.constants.O_NOFOLLOW, 0);
    }
    return open(name, flags, ...rest);
  } });
  assert.throws(() => f.open(), /retained file identity changed/);
  assert.equal(switched, true);
});
test("ancestor replacement during leaf open refuses original retained lineage", (t) => {
  const f = fixture(t); const open = fs.openSync; let switched = false;
  const moved = f.directory + ".old";
  t.after(() => fs.rmSync(moved, { recursive: true, force: true }));
  instrument(t, { openSync(name, flags, ...rest) {
    if (name === f.originalPath && !switched) {
      switched = true; fs.renameSync(f.directory, moved);
      fs.cpSync(moved, f.directory, { recursive: true });
    }
    return open(name, flags, ...rest);
  } });
  assert.throws(() => f.open(), refusal); assert.equal(switched, true);
});
test("read refusal closes every acquired descriptor and never accepts a retry", (t) => {
  const f = fixture(t); const open = fs.openSync; const close = fs.closeSync; const read = fs.readSync;
  const held = new Set(); let refuse = false;
  instrument(t, {
    openSync(...args) { const fd = open(...args); held.add(fd); return fd; },
    closeSync(fd) { held.delete(fd); return close(fd); },
    readSync(...args) { if (refuse) throw new Error("injected read refusal"); return read(...args); },
  });
  const owner = f.open(); assert(held.size > 3);
  const snapshot = f.snapshot(); const { cache } = f.cache(snapshot.snapshotPath);
  refuse = true;
  assert.throws(() => owner.identify(cache), /injected read refusal/);
  assert.equal(held.size, 0);
  assert.throws(() => owner.recheck(), /closed or invalidated/);
});
test("constructor digest refusal and ordinary close release all opened descriptors", (t) => {
  const f = fixture(t); const open = fs.openSync; const close = fs.closeSync; const held = new Set();
  instrument(t, {
    openSync(...args) { const fd = open(...args); held.add(fd); return fd; },
    closeSync(fd) { assert(held.delete(fd)); return close(fd); },
  });
  assert.throws(() => f.open({ expectedSha256: "2".repeat(64) }), /original digest differs/);
  assert.equal(held.size, 0);
  const owner = f.open(); assert(held.size > 3); owner.close(); assert.equal(held.size, 0);
});
for (const nested of ["recheck", "close"]) {
  test(`swallowed reentrant ${nested} refusal poisons the outer observation`, (t) => {
    const f = identified(t); const read = fs.readSync; let invoked = false;
    instrument(t, { readSync(...args) {
      if (!invoked) {
        invoked = true;
        assert.throws(() => f.owner[nested](), /invalidated|close during observation/);
      }
      return read(...args);
    } });
    poisoned(f.owner, () => f.owner.recheck()); assert.equal(invoked, true);
  });
  test(`swallowed ${nested} during the final held-directory fstat cannot leave outer success`, (t) => {
    const f = identified(t); const fstat = fs.fstatSync;
    let count = 0; let finalCount; let invoked = false;
    instrument(t, { fstatSync(...args) {
      count++;
      const observed = fstat(...args);
      if (count === finalCount) {
        assert.equal(observed.isDirectory(), true);
        invoked = true;
        assert.throws(() => f.owner[nested](), /invalidated|close during observation/);
      }
      return observed;
    } });
    f.owner.recheck(); finalCount = count; count = 0;
    poisoned(f.owner, () => f.owner.recheck());
    assert.equal(invoked, true);
  });
}
test("cleanup attempts every descriptor even when a close reports failure", (t) => {
  const f = fixture(t); const open = fs.openSync; const close = fs.closeSync;
  const held = new Set(); let threw = false;
  instrument(t, {
    openSync(...args) { const fd = open(...args); held.add(fd); return fd; },
    closeSync(fd) {
      held.delete(fd); close(fd);
      if (!threw) { threw = true; throw new Error("injected close completion error"); }
    },
  });
  const owner = f.open();
  assert.throws(() => owner.close(), (error) => error instanceof AggregateError
    && error.errors.length === 1 && /injected close completion error/.test(error.errors[0].message));
  assert.equal(held.size, 0); owner.close();
  assert.throws(() => owner.recheck(), /closed or invalidated/);
});
test("close detaches before I/O and never closes a reclaimed descriptor number on retry", (t) => {
  const f = fixture(t); const open = fs.openSync; const close = fs.closeSync;
  const replacement = join(f.directory, "replacement"); fs.writeFileSync(replacement, "new descriptor owner");
  const held = new Set(); let owner; let reclaimed;
  instrument(t, {
    openSync(...args) { const fd = open(...args); held.add(fd); return fd; },
    closeSync(fd) {
      assert(held.delete(fd)); close(fd);
      if (reclaimed === undefined) {
        reclaimed = open(replacement, fs.constants.O_RDONLY);
        assert.equal(reclaimed, fd, "actual task-owned descriptor number must be reused");
        // Reentry occurs after the real close and before it reports failure.
        owner.close();
        throw new Error("close succeeded then reported failure");
      }
    },
  });
  t.after(() => { if (reclaimed !== undefined) close(reclaimed); });
  owner = f.open();
  assert.throws(() => owner.close(), AggregateError);
  assert.equal(held.size, 0);
  assert.equal(fs.fstatSync(reclaimed).ino, fs.statSync(replacement).ino);
  owner.close();
  assert.equal(fs.fstatSync(reclaimed).ino, fs.statSync(replacement).ino);
});
