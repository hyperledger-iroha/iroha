// Pure cache-identity controls using inert records in Node's actual require.cache.
// No .node file is opened, required, or executed. These tests do not establish
// native bytes, source custody, file ownership, mapped memory, or qualification.
import assert from "node:assert/strict";
import Module, { createRequire } from "node:module";
import { resolve, sep } from "node:path";
import test from "node:test";
import { NativeCacheObservation } from "../../../scripts/sorafs_javascript_native_cache.mjs";

const require = createRequire(import.meta.url);
let serial = 0;
const refusal = /JavaScript native cache:/;

// Tests are synchronous and sequential because the cache is process-global.
// Restore every original own descriptor even when a negative assertion fails.
function fixture(t) {
  const cache = require.cache;
  const originals = Object.getOwnPropertyDescriptors(cache);
  assert.equal(Module._cache, cache);
  assert.equal(Reflect.ownKeys(cache).some((key) => typeof key === "string" && key.endsWith(".node")), false);
  t.after(() => {
    Module._cache = cache;
    for (const key of Reflect.ownKeys(cache)) {
      if (!Object.hasOwn(originals, key)) assert.equal(Reflect.deleteProperty(cache, key), true);
    }
    Object.defineProperties(cache, originals);
    assert.deepEqual(Object.getOwnPropertyDescriptors(cache), originals);
  });
  return {
    cache,
    path(suffix = ".node") {
      return resolve("inert-cache-observation", `never-opened-${serial++}${suffix}`);
    },
    put(key, value) {
      Object.defineProperty(cache, key, { value, configurable: true, enumerable: true, writable: true });
    },
  };
}

function record(filename, binding = { nativeOperation() {}, version: 1 }) {
  return { id: filename, filename, loaded: true, exports: binding };
}

function loaded(t, binding) {
  const context = fixture(t);
  const owner = new NativeCacheObservation();
  const filename = context.path();
  const module = record(filename, binding);
  context.put(filename, module);
  return { ...context, owner, filename, module, binding: module.exports };
}

function poisoned(owner, invoke, pattern = refusal) {
  assert.throws(invoke, pattern);
  assert.throws(() => owner.recheck(), /invalidated by an earlier refusal/);
  assert.throws(() => owner.identify(() => ({})), /invalidated by an earlier refusal/);
}

test("same already-loaded object is observed without a second getter call", (t) => {
  const f = loaded(t);
  let calls = 0;
  const observation = f.owner.identify(() => { calls += 1; return f.binding; });
  assert.deepEqual(observation, { filename: f.filename, exportNames: ["nativeOperation", "version"] });
  assert.equal(calls, 1);
  assert.equal(Object.isFrozen(observation), true);
  assert.equal(Object.isFrozen(observation.exportNames), true);
  assert.throws(() => observation.exportNames.push("forged"), TypeError);
  assert.deepEqual(f.owner.recheck(), observation);
  assert.deepEqual(f.owner.recheck(), observation);
  assert.equal(calls, 1);
});

test("ordinary JavaScript cache additions are permitted around the native observation", (t) => {
  const f = loaded(t);
  f.put(f.path(".js"), { exports: {} });
  f.owner.identify(() => f.binding);
  f.put(f.path(".js"), { exports: {} });
  assert.equal(f.owner.recheck().filename, f.filename);
});

test("null-prototype module and binding data objects are supported", (t) => {
  const binding = Object.assign(Object.create(null), { call() {} });
  const f = loaded(t, binding);
  Object.setPrototypeOf(f.module, null);
  assert.deepEqual(f.owner.identify(() => binding).exportNames, ["call"]);
});

test("a preexisting native cache record is refused before observation starts", (t) => {
  const f = fixture(t);
  const filename = f.path();
  f.put(filename, record(filename));
  assert.throws(() => new NativeCacheObservation(), /already loaded/);
});

test("an absent load cannot be manufactured by invoking the supplied getter", (t) => {
  const f = fixture(t);
  const owner = new NativeCacheObservation();
  let calls = 0;
  poisoned(owner, () => owner.identify(() => {
    calls += 1;
    const filename = f.path();
    const module = record(filename);
    f.put(filename, module);
    return module.exports;
  }), /exactly one already loaded/);
  assert.equal(calls, 0);
});

for (const sameBinding of [false, true]) {
  test(`two new native records are ambiguous even with same binding=${sameBinding}`, (t) => {
    const f = loaded(t);
    const second = f.path();
    f.put(second, record(second, sameBinding ? f.binding : undefined));
    let calls = 0;
    poisoned(f.owner, () => f.owner.identify(() => { calls += 1; return f.binding; }), /exactly one/);
    assert.equal(calls, 0);
  });
}

test("recheck before identify irreversibly refuses the owner", (t) => {
  const f = loaded(t);
  poisoned(f.owner, () => f.owner.recheck(), /has not been identified/);
});

test("a second identify call irreversibly refuses the owner", (t) => {
  const f = loaded(t);
  f.owner.identify(() => f.binding);
  poisoned(f.owner, () => f.owner.identify(() => f.binding), /already identified/);
});

for (const location of ["baseline", "new native", "new JavaScript"]) {
  test(`cache accessor at ${location} is refused without invoking it`, (t) => {
    const f = fixture(t);
    let calls = 0;
    const filename = f.path(location === "new native" ? ".node" : ".js");
    const add = () => Object.defineProperty(f.cache, filename, {
      configurable: true,
      get() { calls += 1; return record(filename); },
    });
    if (location === "baseline") {
      add();
      assert.throws(() => new NativeCacheObservation(), /own data property/);
    } else {
      const owner = new NativeCacheObservation();
      add();
      poisoned(owner, () => owner.identify(() => ({})), /own data property/);
    }
    assert.equal(calls, 0);
  });
}

for (const stage of ["constructor", "identify", "recheck"]) {
  test(`symbol cache key is refused at ${stage}`, (t) => {
    const f = fixture(t);
    const owner = stage === "constructor" ? null : new NativeCacheObservation();
    const filename = f.path();
    const module = record(filename);
    if (stage === "recheck") {
      f.put(filename, module);
      owner.identify(() => module.exports);
    }
    f.put(Symbol("inert cache symbol"), {});
    if (stage === "constructor") assert.throws(() => new NativeCacheObservation(), /cache inventory/);
    else poisoned(owner, () => stage === "identify" ? owner.identify(() => module.exports) : owner.recheck(), /cache inventory/);
  });
}

for (const stage of ["constructor", "identify", "recheck"]) {
  test(`replacement of Node's actual cache owner is refused at ${stage}`, (t) => {
    const f = fixture(t);
    const owner = stage === "constructor" ? null : new NativeCacheObservation();
    const filename = f.path();
    const module = record(filename);
    if (owner) f.put(filename, module);
    if (stage === "recheck") owner.identify(() => module.exports);
    Module._cache = Object.create(null);
    if (stage === "constructor") assert.throws(() => new NativeCacheObservation(), /cache owner was replaced/);
    else poisoned(owner, () => stage === "identify" ? owner.identify(() => module.exports) : owner.recheck(), /cache owner was replaced/);
  });
}

const baselineMutations = {
  delete: (f, key) => { delete f.cache[key]; },
  replace: (f, key) => { f.cache[key] = {}; },
  enumerable: (f, key) => { Object.defineProperty(f.cache, key, { enumerable: false }); },
  writable: (f, key) => { Object.defineProperty(f.cache, key, { writable: false }); },
};
for (const stage of ["identify", "recheck"]) {
  for (const [name, mutate] of Object.entries(baselineMutations)) {
    test(`preexisting JavaScript cache descriptor ${name} mutation is refused at ${stage}`, (t) => {
      const f = fixture(t);
      const original = f.path(".js");
      f.put(original, { exports: {} });
      const owner = new NativeCacheObservation();
      const filename = f.path();
      const module = record(filename);
      f.put(filename, module);
      if (stage === "recheck") owner.identify(() => module.exports);
      mutate(f, original);
      poisoned(owner, () => stage === "identify" ? owner.identify(() => module.exports) : owner.recheck(), /preexisting module cache entry changed/);
    });
  }
}

for (const [label, filename] of [
  ["relative", "never-opened.node"],
  ["dot segment", `${resolve("inert-cache-observation")}${sep}.${sep}never-opened.node`],
  ["parent segment", `${resolve("inert-cache-observation")}${sep}child${sep}..${sep}never-opened.node`],
  ["NUL", `${resolve("inert-cache-observation")}${sep}never\0opened.node`],
]) {
  test(`native filename rejects ${label}`, (t) => {
    const f = fixture(t);
    const owner = new NativeCacheObservation();
    const module = record(filename);
    f.put(filename, module);
    poisoned(owner, () => owner.identify(() => module.exports), /absolute and canonical/);
  });
}

for (const value of [null, undefined, false, 1, "module", () => {}]) {
  test(`native cache value refuses non-object ${String(value)}`, (t) => {
    const f = loaded(t);
    f.cache[f.filename] = value;
    poisoned(f.owner, () => f.owner.identify(() => f.binding), /not a module/);
  });
}

for (const field of ["id", "filename", "loaded", "exports"]) {
  for (const shape of ["missing", "inherited", "accessor"]) {
    test(`module ${field} ${shape} is refused without getter execution`, (t) => {
      const f = loaded(t);
      let calls = 0;
      const original = f.module[field];
      delete f.module[field];
      if (shape === "inherited") Object.setPrototypeOf(f.module, { [field]: original });
      if (shape === "accessor") Object.defineProperty(f.module, field, {
        configurable: true,
        get() { calls += 1; return original; },
      });
      poisoned(f.owner, () => f.owner.identify(() => f.binding), /own data property/);
      assert.equal(calls, 0);
    });
  }
}

for (const [field, value] of [["id", "other.node"], ["filename", "other.node"], ["loaded", false], ["loaded", 1]]) {
  test(`module ${field}=${value} does not describe the completed native load`, (t) => {
    const f = loaded(t);
    f.module[field] = value;
    poisoned(f.owner, () => f.owner.identify(() => f.binding), /identity or completed load differs/);
  });
}

for (const value of [null, undefined, true, 1, "binding", () => {}]) {
  test(`exports refuses non-object ${String(value)}`, (t) => {
    const f = loaded(t);
    f.module.exports = value;
    poisoned(f.owner, () => f.owner.identify(() => value), /not a binding object/);
  });
}

for (const [label, binding, pattern] of [
  ["empty", {}, /exports inventory/],
  ["data only", { version: 1 }, /no callable exports/],
  ["inherited callable", Object.create({ call() {} }), /exports inventory/],
  ["symbol", { call() {}, [Symbol("hidden")]: 1 }, /exports inventory/],
]) {
  test(`exports rejects ${label}`, (t) => {
    const f = loaded(t, binding);
    poisoned(f.owner, () => f.owner.identify(() => binding), pattern);
  });
}

test("an export accessor cannot manufacture a callable", (t) => {
  let calls = 0;
  const binding = {};
  Object.defineProperty(binding, "call", { configurable: true, get() { calls += 1; return () => {}; } });
  const f = loaded(t, binding);
  poisoned(f.owner, () => f.owner.identify(() => binding), /own data property/);
  assert.equal(calls, 0);
});

for (const get of [null, false, 7, {}, "getter"]) {
  test(`identify refuses non-callable getter ${String(get)}`, (t) => {
    const f = loaded(t);
    poisoned(f.owner, () => f.owner.identify(get), /installed getter differs/);
  });
}

test("structurally equal exports from the getter are not the loaded binding", (t) => {
  const f = loaded(t);
  poisoned(f.owner, () => f.owner.identify(() => ({ ...f.binding })), /installed getter differs/);
});

test("a throwing getter permanently refuses the observation", (t) => {
  const f = loaded(t);
  const error = new Error("inert getter failed");
  poisoned(f.owner, () => f.owner.identify(() => { throw error; }), error);
});

const getterMutations = {
  "second native load": (f) => { const path = f.path(); f.put(path, record(path)); },
  "deleted load": (f) => { delete f.cache[f.filename]; },
  "replaced cache record": (f) => { f.cache[f.filename] = record(f.filename, f.binding); },
  "changed loaded flag": (f) => { f.module.loaded = false; },
  "replaced callable": (f) => { f.binding.nativeOperation = () => {}; },
  "extra export": (f) => { f.binding.added = 1; },
  "changed cache owner": () => { Module._cache = Object.create(null); },
};
for (const [name, mutate] of Object.entries(getterMutations)) {
  test(`getter ${name} is refused by the immediate recheck`, (t) => {
    const f = loaded(t);
    poisoned(f.owner, () => f.owner.identify(() => { mutate(f); return f.binding; }));
  });
}

for (const method of ["recheck", "identify"]) {
  test(`getter cannot erase a caught reentrant ${method} refusal`, (t) => {
    const f = loaded(t);
    let nestedRefused = false;
    poisoned(f.owner, () => f.owner.identify(() => {
      try {
        if (method === "identify") f.owner.identify(() => f.binding);
        else f.owner.recheck();
      } catch (error) {
        assert.match(error.message, refusal);
        nestedRefused = true;
      }
      return f.binding;
    }));
    assert.equal(nestedRefused, true);
  });
}

const cacheMutations = {
  delete: (f) => { delete f.cache[f.filename]; },
  replacement: (f) => { f.cache[f.filename] = record(f.filename, f.binding); },
  enumerable: (f) => { Object.defineProperty(f.cache, f.filename, { enumerable: false }); },
  writable: (f) => { Object.defineProperty(f.cache, f.filename, { writable: false }); },
  accessor: (f) => { Object.defineProperty(f.cache, f.filename, { get() { assert.fail("cache getter executed"); } }); },
  extra: (f) => { const path = f.path(); f.put(path, record(path, f.binding)); },
};
for (const [name, mutate] of Object.entries(cacheMutations)) {
  test(`recheck refuses native cache ${name} mutation`, (t) => {
    const f = loaded(t);
    f.owner.identify(() => f.binding);
    mutate(f);
    poisoned(f.owner, () => f.owner.recheck());
  });
}

for (const field of ["id", "filename", "loaded", "exports"]) {
  for (const change of ["value", "writable", "enumerable", "configurable", "accessor"]) {
    test(`recheck refuses module ${field} ${change} mutation`, (t) => {
      const f = loaded(t);
      f.owner.identify(() => f.binding);
      if (change === "value") f.module[field] = field === "exports" ? { ...f.binding } : null;
      else if (change === "accessor") Object.defineProperty(f.module, field, { get() { assert.fail("module getter executed"); } });
      else Object.defineProperty(f.module, field, { [change]: false });
      poisoned(f.owner, () => f.owner.recheck());
    });
  }
}

const exportMutations = {
  callable: (binding) => { binding.nativeOperation = () => {}; },
  number: (binding) => { binding.version += 1; },
  add: (binding) => { binding.newField = true; },
  delete: (binding) => { delete binding.version; },
  order: (binding) => { const call = binding.nativeOperation; delete binding.nativeOperation; binding.nativeOperation = call; },
  prototype: (binding) => { Object.setPrototypeOf(binding, Object.create(null)); },
  writable: (binding) => { Object.defineProperty(binding, "nativeOperation", { writable: false }); },
  enumerable: (binding) => { Object.defineProperty(binding, "nativeOperation", { enumerable: false }); },
  configurable: (binding) => { Object.defineProperty(binding, "nativeOperation", { configurable: false }); },
  accessor: (binding) => { Object.defineProperty(binding, "version", { get() { assert.fail("export getter executed"); } }); },
  symbol: (binding) => { binding[Symbol("new export")] = 1; },
};
for (const [name, mutate] of Object.entries(exportMutations)) {
  test(`recheck refuses export ${name} mutation`, (t) => {
    const f = loaded(t);
    f.owner.identify(() => f.binding);
    mutate(f.binding);
    poisoned(f.owner, () => f.owner.recheck());
  });
}

test("restoring refused module fields does not revive the owner", (t) => {
  const f = loaded(t);
  f.owner.identify(() => f.binding);
  f.module.loaded = false;
  assert.throws(() => f.owner.recheck(), refusal);
  f.module.loaded = true;
  poisoned(f.owner, () => f.owner.recheck(), /invalidated/);
});

test("unchanged NaN data exports retain their exact JavaScript value", (t) => {
  const f = loaded(t, { call() {}, value: NaN });
  f.owner.identify(() => f.binding);
  assert.equal(f.owner.recheck().filename, f.filename);
});

for (const [before, after, label] of [[0, -0, "positive to negative"], [-0, 0, "negative to positive"]]) {
  test(`recheck refuses ${label} zero data export changes`, (t) => {
    const f = loaded(t, { call() {}, value: before });
    f.owner.identify(() => f.binding);
    f.binding.value = after;
    poisoned(f.owner, () => f.owner.recheck(), /native exports changed/);
  });
}

function fillCache(f, total) {
  const initial = Reflect.ownKeys(f.cache).length;
  for (let i = initial; i < total; i += 1) f.put(f.path(".js"), {});
}

test("cache exact 4096-entry ceiling admits one native record", (t) => {
  const f = fixture(t);
  fillCache(f, 4095);
  const owner = new NativeCacheObservation();
  const filename = f.path();
  const module = record(filename);
  f.put(filename, module);
  assert.equal(Reflect.ownKeys(f.cache).length, 4096);
  assert.equal(owner.identify(() => module.exports).filename, filename);
  assert.equal(owner.recheck().filename, filename);
});

test("constructor admits exactly 4096 ordinary entries and refuses 4097", (t) => {
  const f = fixture(t);
  fillCache(f, 4096);
  assert.doesNotThrow(() => new NativeCacheObservation());
  f.put(f.path(".js"), {});
  assert.throws(() => new NativeCacheObservation(), /cache inventory/);
});

for (const stage of ["identify", "recheck"]) {
  test(`cache overflow is refused and poisons at ${stage}`, (t) => {
    const f = loaded(t);
    if (stage === "recheck") f.owner.identify(() => f.binding);
    fillCache(f, 4097);
    poisoned(f.owner, () => stage === "identify" ? f.owner.identify(() => f.binding) : f.owner.recheck(), /cache inventory/);
  });
}

function bindingOfSize(count) {
  return Object.fromEntries(Array.from({ length: count }, (_, i) => [`export${i}`, i === 0 ? () => {} : i]));
}

test("binding exact 1024-own-export ceiling is admitted", (t) => {
  const f = loaded(t, bindingOfSize(1024));
  assert.equal(f.owner.identify(() => f.binding).exportNames.length, 1024);
  assert.equal(f.owner.recheck().exportNames.length, 1024);
});

for (const stage of ["identify", "recheck"]) {
  test(`binding 1025-own-export overflow is refused and poisons at ${stage}`, (t) => {
    const f = loaded(t, bindingOfSize(stage === "identify" ? 1025 : 1024));
    if (stage === "recheck") {
      f.owner.identify(() => f.binding);
      f.binding.oneTooMany = true;
    }
    poisoned(f.owner, () => stage === "identify" ? f.owner.identify(() => f.binding) : f.owner.recheck(), /exports inventory/);
  });
}

function filenameOfLength(length) {
  const prefix = `${resolve("inert-cache-observation")}${sep}`;
  return `${prefix}${"a".repeat(length - prefix.length - 5)}.node`;
}

test("an exact 4096-code-unit native cache filename is admitted as identity metadata", (t) => {
  const f = fixture(t);
  const owner = new NativeCacheObservation();
  const filename = filenameOfLength(4096);
  const module = record(filename);
  assert.equal(filename.length, 4096);
  f.put(filename, module);
  assert.equal(owner.identify(() => module.exports).filename, filename);
  assert.equal(owner.recheck().filename, filename);
});

test("an exact 4096-code-unit ordinary cache key is admitted at construction", (t) => {
  const f = fixture(t);
  f.put("j".repeat(4096), {});
  assert.doesNotThrow(() => new NativeCacheObservation());
});

for (const stage of ["constructor", "identify", "recheck"]) {
  test(`cache key 4097-code-unit overflow is refused at ${stage}`, (t) => {
    const f = fixture(t);
    const owner = stage === "constructor" ? null : new NativeCacheObservation();
    const filename = stage === "identify" ? filenameOfLength(4097) : f.path();
    const module = record(filename);
    if (stage !== "constructor") f.put(filename, module);
    if (stage === "recheck") owner.identify(() => module.exports);
    if (stage !== "identify") f.put("j".repeat(4097), {});
    if (stage === "constructor") assert.throws(() => new NativeCacheObservation(), /cache inventory/);
    else poisoned(owner, () => stage === "identify" ? owner.identify(() => module.exports) : owner.recheck(), /cache inventory/);
  });
}

for (const [label, name] of [["ASCII", "n".repeat(256)], ["surrogate pairs", "\u{1f642}".repeat(128)]]) {
  test(`exact 256-code-unit export name is admitted with ${label}`, (t) => {
    assert.equal(name.length, 256);
    const f = loaded(t, { [name]() {} });
    assert.deepEqual(f.owner.identify(() => f.binding).exportNames, [name]);
    assert.deepEqual(f.owner.recheck().exportNames, [name]);
  });
}

for (const [label, name] of [["empty", ""], ["257 ASCII code units", "n".repeat(257)], ["257 Unicode code units", `${"\u{1f642}".repeat(128)}x`]]) {
  for (const stage of ["identify", "recheck"]) {
    test(`export name ${label} is refused at ${stage}`, (t) => {
      const f = loaded(t, stage === "identify" ? { [name]() {} } : undefined);
      if (stage === "recheck") {
        f.owner.identify(() => f.binding);
        f.binding[name] = () => {};
      }
      poisoned(f.owner, () => stage === "identify" ? f.owner.identify(() => f.binding) : f.owner.recheck(), /exports inventory/);
    });
  }
}
