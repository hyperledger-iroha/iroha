// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

// Pure locked-dependency/runtime minimum contract. No I/O or SDK/native imports.
import assert from "node:assert/strict";

const PACKAGE_NAME = "(?:@[a-z0-9][a-z0-9._-]*/)?[a-z0-9][a-z0-9._-]*";
const DEPENDENCY_NAME = new RegExp(`^${PACKAGE_NAME}$`, "u");
const LOCK_LOCATION = new RegExp(`^node_modules/${PACKAGE_NAME}(?:/node_modules/${PACKAGE_NAME})*$`, "u");

function versionParts(value, complete = false) {
  const pattern = complete
    ? /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/u
    : /^(0|[1-9]\d*)(?:\.(0|[1-9]\d*))?(?:\.(0|[1-9]\d*))?$/u;
  assert.equal(typeof value, "string", "Node version must be a string");
  const match = pattern.exec(value);
  assert.ok(match, `unsupported Node version spelling: ${value}`);
  const parts = match.slice(1).map((part) => Number(part ?? 0));
  assert.ok(parts.every(Number.isSafeInteger), "Node version exceeds exact integer bounds");
  return parts;
}

function atLeast(actual, required) {
  for (let index = 0; index < 3; index += 1) {
    if (actual[index] !== required[index]) return actual[index] > required[index];
  }
  return true;
}

function coversFloor(range, floor) {
  assert.equal(typeof range, "string", "dependency Node engine must be a string");
  // This is the reviewed engine grammar in our locked production closure, not
  // a general semver implementation. An unbounded SDK range needs an accepted
  // unbounded dependency tail; new range forms require explicit review.
  const alternatives = range.split(" || ");
  const tails = alternatives.map((alternative) => {
    const match = /^(>= ?|\^)([0-9.]+)$/u.exec(alternative);
    assert.ok(match, `unreviewed dependency Node engine range: ${range}`);
    const parts = versionParts(match[2], match[1] === "^");
    return match[1].startsWith(">=") ? parts : null;
  });
  return tails.some((tail) => tail !== null && atLeast(floor, tail));
}

function dependencies(row, label) {
  assert.ok(row && typeof row === "object" && !Array.isArray(row), `${label} must be an object`);
  for (const field of ["optionalDependencies", "peerDependencies", "peerDependenciesMeta", "bundleDependencies", "bundledDependencies", "link"]) {
    assert.equal(row[field], undefined, `${label} introduces an unreviewed ${field} edge`);
  }
  const selected = row.dependencies ?? {};
  assert.ok(selected && typeof selected === "object" && !Array.isArray(selected), `${label} dependencies must be an object`);
  for (const [name, range] of Object.entries(selected)) {
    assert.match(name, DEPENDENCY_NAME);
    assert.equal(typeof range, "string", `${label} dependency range must be a string`);
    assert.ok(range.length > 0, `${label} dependency range must not be empty`);
  }
  return selected;
}

function matchesSelection(actualVersion, selectedRange) {
  assert.equal(typeof selectedRange, "string");
  const caret = selectedRange.startsWith("^");
  const required = versionParts(caret ? selectedRange.slice(1) : selectedRange, true);
  const actual = versionParts(actualVersion, true);
  if (!caret) return actual.every((part, index) => part === required[index]);
  const upper = [...required];
  const pivot = required.findIndex((part) => part !== 0);
  const index = pivot === -1 ? 2 : pivot;
  upper[index] += 1;
  assert.ok(Number.isSafeInteger(upper[index]), "dependency caret upper bound exceeds exact integers");
  upper.fill(0, index + 1);
  return atLeast(actual, required) && !atLeast(actual, upper);
}

function resolveDependency(packages, owner, name) {
  let parent = owner;
  for (;;) {
    const location = `${parent ? `${parent}/` : ""}node_modules/${name}`;
    if (Object.hasOwn(packages, location)) return location;
    if (!parent) throw new Error(`missing production dependency: ${owner || "root"} -> ${name}`);
    const marker = parent.lastIndexOf("/node_modules/");
    parent = marker === -1 ? "" : parent.slice(0, marker);
  }
}

/** Check the published lower bound against the actual locked production closure. */
export function validateNodeEngineContract(pkg, lock, runtimeVersion) {
  assert.equal(lock?.lockfileVersion, 3, "Node engine guard requires package-lock V3");
  assert.ok(lock.packages && typeof lock.packages === "object" && !Array.isArray(lock.packages));
  // Reject every unreviewed location, including dev rows that could shadow a
  // production import through a scope-directory Node search slot.
  for (const location of Object.keys(lock.packages)) {
    assert.ok(location === "" || (location.length <= 4096 && LOCK_LOCATION.test(location)),
      `unreviewed lock package location: ${location}`);
  }
  const root = lock.packages[""];
  assert.equal(root?.name, pkg.name, "package and lock names differ");
  assert.equal(root?.version, pkg.version, "package and lock versions differ");
  assert.equal(typeof pkg.engines?.node, "string", "SDK must declare a Node lower bound");
  assert.match(pkg.engines.node, /^>=\d+\.\d+\.\d+$/u, "SDK must declare one canonical >=major.minor.patch bound");
  assert.deepEqual(root.engines, pkg.engines, "package and lock engine contracts differ");
  const floor = versionParts(pkg.engines.node.slice(2), true);
  assert.deepEqual(dependencies(root, "lock root"), dependencies(pkg, "package"), "package and lock dependency selections differ");
  const reached = new Set();
  const queue = [["", root]];
  for (let index = 0; index < queue.length; index += 1) {
    const [owner, row] = queue[index];
    for (const [name, selectedRange] of Object.entries(dependencies(row, owner || "root"))) {
      const location = resolveDependency(lock.packages, owner, name);
      const dependency = lock.packages[location];
      assert.ok(matchesSelection(dependency.version, selectedRange),
        `${location} version ${dependency.version} does not match production selection ${selectedRange}`);
      if (reached.has(location)) continue;
      assert.ok(dependency && dependency.dev !== true && dependency.devOptional !== true,
        `${location} is a production dependency incorrectly marked development-only`);
      if (dependency.engines !== undefined) {
        assert.ok(dependency.engines && typeof dependency.engines === "object" && !Array.isArray(dependency.engines),
          `${location} engines must be an object`);
      }
      if (dependency.engines?.node !== undefined) {
        assert.ok(coversFloor(dependency.engines.node, floor),
          `${location} requires ${dependency.engines.node}, outside published ${pkg.engines.node}`);
      }
      reached.add(location);
      queue.push([location, dependency]);
    }
  }
  for (const [location, row] of Object.entries(lock.packages)) {
    assert.ok(row && typeof row === "object" && !Array.isArray(row), `${location} lock row must be an object`);
    if (location && row.dev !== true) {
      assert.ok(reached.has(location), `${location} is an unowned production lock entry`);
    }
  }
  const actual = versionParts(runtimeVersion, true);
  assert.ok(atLeast(actual, floor), `current Node ${runtimeVersion} is below SDK ${pkg.engines.node}`);
  return Object.freeze({ minimum: floor.join("."), productionDependencies: reached.size });
}
