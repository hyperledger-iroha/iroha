import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";

import {
  probeNativeBindingExports,
  publishNativeBinding,
  recoverNativeBindingPublication,
  REQUIRED_NATIVE_EXPORTS,
} from "../scripts/copy-native.mjs";
import { verifyNativeBinding } from "../src/native.js";

const PLATFORM = "linux";
const ARCH = "x64";
const PLATFORM_KEY = `${PLATFORM}-${ARCH}`;
const NATIVE_FILENAME = "iroha_js_host.node";
const MANIFEST_FILENAME = "iroha_js_host.checksums.json";
const COPY_NATIVE_SCRIPT = path.resolve("scripts/copy-native.mjs");
const SOURCE_REVISION = "a".repeat(40);
const SOURCE_TREE_DIGEST = "b".repeat(64);

function fixtureBuildProvenance(source, cargoProfile = "debug") {
  return {
    version: 2,
    cargo_profile: cargoProfile,
    native_sha256: createHash("sha256").update(readFileSync(source)).digest("hex"),
    source_git_revision: SOURCE_REVISION,
    source_tree_clean: true,
    source_tree_sha256: SOURCE_TREE_DIGEST,
  };
}

const delay = (milliseconds) => new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));

async function waitForFile(file, child, timeoutMs = 10_000) {
  const started = Date.now();
  while (!existsSync(file)) {
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new Error(`copy-native crash worker exited before reaching its phase: ${child.stderrText}`);
    }
    if (Date.now() - started > timeoutMs) {
      throw new Error(`timed out waiting for copy-native crash worker marker: ${file}`);
    }
    await delay(10);
  }
}

function spawnCrashPublisher(layout, phase) {
  const marker = path.join(layout.root, `phase-${phase}`);
const source = String.raw`
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { publishNativeBinding } from ${JSON.stringify(pathToFileURL(COPY_NATIVE_SCRIPT).href)};
const blocker = new Int32Array(new SharedArrayBuffer(4));
await publishNativeBinding({
  source: process.env.SOURCE,
  destDir: process.env.DEST_DIR,
  platform: ${JSON.stringify(PLATFORM)},
  arch: ${JSON.stringify(ARCH)},
  signNative() {},
  probeBinding() {},
  cargoProfile: "debug",
  readBuildProvenance(source) {
    return {
      version: 2,
      cargo_profile: "debug",
      native_sha256: createHash("sha256").update(readFileSync(source)).digest("hex"),
      source_git_revision: ${JSON.stringify(SOURCE_REVISION)},
      source_tree_clean: true,
      source_tree_sha256: ${JSON.stringify(SOURCE_TREE_DIGEST)},
    };
  },
  log() {},
  phaseHook(phase) {
    if (phase !== process.env.TARGET_PHASE) return;
    writeFileSync(process.env.MARKER, phase, { flag: "wx" });
    Atomics.wait(blocker, 0, 0);
  },
});
`;
  const child = spawn(process.execPath, ["--input-type=module", "--eval", source], {
    env: {
      ...process.env,
      DEST_DIR: layout.destDir,
      MARKER: marker,
      SOURCE: layout.source,
      TARGET_PHASE: phase,
    },
    stdio: ["ignore", "ignore", "pipe"],
  });
  child.stderrText = "";
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    child.stderrText += chunk;
  });
  return { child, marker };
}

function waitForExit(child) {
  return new Promise((resolveExit, rejectExit) => {
    child.once("error", rejectExit);
    child.once("exit", (code, signal) => resolveExit({ code, signal }));
  });
}

async function killPublisherAtPhase(layout, phase) {
  const { child, marker } = spawnCrashPublisher(layout, phase);
  await waitForFile(marker, child);
  const exited = waitForExit(child);
  assert.equal(child.kill("SIGKILL"), true);
  const exit = await exited;
  assert.equal(exit.code, null);
  assert.equal(exit.signal, "SIGKILL");
  const lockPath = path.join(layout.destDir, ".build-dist.lock");
  assert.equal(existsSync(lockPath), true, "hard-killed publisher must leave its lock");
  const stale = new Date(Date.now() - 10 * 60_000);
  utimesSync(lockPath, stale, stale);
}

function createLayout(t) {
  const root = mkdtempSync(path.join(os.tmpdir(), "iroha-js-copy-native-"));
  const source = path.join(root, "libiroha_js_host.so");
  const destDir = path.join(root, "native");
  t.after(() => rmSync(root, { recursive: true, force: true }));
  return {
    root,
    source,
    destDir,
    bindingPath: path.join(destDir, NATIVE_FILENAME),
    manifestPath: path.join(destDir, MANIFEST_FILENAME),
  };
}

function publicationOptions(layout, overrides = {}) {
  return {
    source: layout.source,
    destDir: layout.destDir,
    platform: PLATFORM,
    arch: ARCH,
    signNative() {},
    probeBinding() {},
    log() {},
    cargoProfile: "debug",
    readBuildProvenance(source) {
      return fixtureBuildProvenance(source);
    },
    ...overrides,
  };
}

function publicationArtifacts(destDir) {
  if (!existsSync(destDir)) return [];
  return readdirSync(destDir)
    .filter(
      (name) => name.startsWith(".iroha-js-host-") || name === ".build-dist.lock",
    )
    .sort();
}

function snapshotPair(layout) {
  return {
    binding: readFileSync(layout.bindingPath),
    manifest: readFileSync(layout.manifestPath),
  };
}

function assertVerifiedPair(layout, expectedBytes) {
  assert.deepEqual(readFileSync(layout.bindingPath), expectedBytes);
  const verification = verifyNativeBinding(layout.bindingPath, {
    manifestPath: layout.manifestPath,
    platformKey: PLATFORM_KEY,
  });
  assert.equal(verification.ok, true);
  assert.deepEqual(publicationArtifacts(layout.destDir), []);
  return verification;
}

test("native publication replaces an existing pair repeatably without rename-overwrite", async (t) => {
  const layout = createLayout(t);
  const first = Buffer.from("first verified native binding");
  const second = Buffer.from("second verified native binding");
  const probes = [];
  const options = publicationOptions(layout, {
    probeBinding(bindingPath, requiredExports) {
      probes.push({ bindingPath, requiredExports: [...requiredExports] });
      assert.deepEqual(requiredExports, REQUIRED_NATIVE_EXPORTS);
    },
  });

  writeFileSync(layout.source, first);
  const firstResult = await publishNativeBinding(options);
  assert.equal(firstResult.platformKey, PLATFORM_KEY);
  assertVerifiedPair(layout, first);
  assert.deepEqual(
    JSON.parse(readFileSync(layout.manifestPath, "utf8")).entries[PLATFORM_KEY],
    {
      sha256: firstResult.sha256,
      cargo_profile: "debug",
      source_git_revision: SOURCE_REVISION,
      source_tree_clean: true,
      source_tree_sha256: SOURCE_TREE_DIGEST,
    },
  );

  // A second publication exercises the Windows-compatible backup-first path:
  // neither rename targets an existing public filename.
  writeFileSync(layout.source, second);
  const secondResult = await publishNativeBinding(options);
  assert.notEqual(secondResult.sha256, firstResult.sha256);
  assertVerifiedPair(layout, second);
  assert.equal(probes.length, 2);
  assert.ok(probes.every(({ bindingPath }) => bindingPath.includes(".iroha-js-host-")));
});

test("same-source publication is idempotent after a fresh probe and returns the exact digest", async (t) => {
  const layout = createLayout(t);
  const bytes = Buffer.from("same verified native binding");
  const probes = [];
  const logs = [];
  const options = publicationOptions(layout, {
    probeBinding(bindingPath) {
      probes.push(bindingPath);
    },
    log(message) {
      logs.push(message);
    },
  });
  writeFileSync(layout.source, bytes);
  const first = await publishNativeBinding(options);
  const second = await publishNativeBinding(options);

  assert.equal(second.sha256, first.sha256);
  assert.match(second.sha256, /^[0-9a-f]{64}$/u);
  assert.equal(probes.length, 2, "an unchanged candidate must still pass its fresh addon probe");
  assert.ok(probes.every((bindingPath) => bindingPath.includes(".iroha-js-host-txn-")));
  assert.match(logs.at(-1), /already matches the verified build output/u);
  assertVerifiedPair(layout, bytes);
});

test("an identical binary with a different valid manifest encoding is idempotent", async (t) => {
  const layout = createLayout(t);
  const bytes = Buffer.from("same native with alternate manifest encoding");
  const logs = [];
  const options = publicationOptions(layout, {
    log(message) {
      logs.push(message);
    },
  });
  writeFileSync(layout.source, bytes);
  const first = await publishNativeBinding(options);

  const alternateManifest = `${JSON.stringify(
    JSON.parse(readFileSync(layout.manifestPath, "utf8")),
  )}\n`;
  writeFileSync(layout.manifestPath, alternateManifest);
  assert.equal(
    verifyNativeBinding(layout.bindingPath, {
      manifestPath: layout.manifestPath,
      platformKey: PLATFORM_KEY,
    }).ok,
    true,
  );

  // This failpoint would make the old implementation enter an unrecoverable
  // transaction because the old and next binaries have the same digest.
  const second = await publishNativeBinding({ ...options, failpoint: "after-native" });
  assert.equal(second.sha256, first.sha256);
  assert.equal(readFileSync(layout.manifestPath, "utf8"), alternateManifest);
  assert.match(logs.at(-1), /already matches the verified build output/u);
  assertVerifiedPair(layout, bytes);
});

test("every publication failpoint restores the exact previous pair and cleans staging", async (t) => {
  for (const failpoint of ["after-backup", "after-native", "after-manifest"]) {
    await t.test(failpoint, async (subtest) => {
      const layout = createLayout(subtest);
      writeFileSync(layout.source, Buffer.from(`old-${failpoint}`));
      await publishNativeBinding(publicationOptions(layout));
      const before = snapshotPair(layout);

      writeFileSync(layout.source, Buffer.from(`new-${failpoint}`));
      await assert.rejects(
        publishNativeBinding(publicationOptions(layout, { failpoint })),
        new RegExp(`injected test failure at ${failpoint}`, "u"),
      );
      assert.deepEqual(snapshotPair(layout), before);
      assertVerifiedPair(layout, before.binding);
    });
  }
});

test("failed first publication leaves no partial binary or checksum manifest", async (t) => {
  for (const failpoint of ["after-backup", "after-native", "after-manifest"]) {
    await t.test(failpoint, async (subtest) => {
      const layout = createLayout(subtest);
      writeFileSync(layout.source, Buffer.from(`new-${failpoint}`));
      await assert.rejects(
        publishNativeBinding(publicationOptions(layout, { failpoint })),
        new RegExp(`injected test failure at ${failpoint}`, "u"),
      );
      assert.equal(existsSync(layout.bindingPath), false);
      assert.equal(existsSync(layout.manifestPath), false);
      assert.deepEqual(publicationArtifacts(layout.destDir), []);
    });
  }
});

test("publisher rejects a stale build-provenance binary before destination mutation", async (t) => {
  const layout = createLayout(t);
  writeFileSync(layout.source, "current-native-output");
  await assert.rejects(
    publishNativeBinding(
      publicationOptions(layout, {
        readBuildProvenance() {
          return {
            ...fixtureBuildProvenance(layout.source),
            native_sha256: "0".repeat(64),
          };
        },
      }),
    ),
    /does not match the compiled binary/u,
  );
  assert.equal(existsSync(layout.destDir), false);
});

test("publisher rejects malformed V2 source seals before destination mutation", async (t) => {
  const layout = createLayout(t);
  writeFileSync(layout.source, "current-native-output");
  const missingDigest = fixtureBuildProvenance(layout.source);
  delete missingDigest.source_tree_sha256;
  for (const malformed of [
    missingDigest,
    {
      ...fixtureBuildProvenance(layout.source),
      source_tree_sha256: "B".repeat(64),
    },
    { ...fixtureBuildProvenance(layout.source), unexpected: true },
    { ...fixtureBuildProvenance(layout.source), version: 1 },
  ]) {
    await assert.rejects(
      publishNativeBinding(
        publicationOptions(layout, {
          readBuildProvenance() {
            return malformed;
          },
        }),
      ),
      /unexpected or missing fields|does not match the compiled binary/u,
    );
    assert.equal(existsSync(layout.destDir), false);
  }
});

test("probe, signing, and post-publish verification failures preserve the old pair", async (t) => {
  for (const [label, override] of [
    ["sign", { signNative() { throw new Error("injected signing failure"); } }],
    ["probe", { probeBinding() { throw new Error("injected probe failure"); } }],
    [
      "post-verify",
      {
        verifyBinding(bindingPath, options) {
          if (
            !bindingPath.includes(".iroha-js-host-txn-") &&
            readFileSync(bindingPath, "utf8") === "new-post-verify"
          ) {
            return { ok: false, status: "injected_post_verify" };
          }
          return verifyNativeBinding(bindingPath, options);
        },
      },
    ],
  ]) {
    await t.test(label, async (subtest) => {
      const layout = createLayout(subtest);
      writeFileSync(layout.source, Buffer.from(`old-${label}`));
      await publishNativeBinding(publicationOptions(layout));
      const before = snapshotPair(layout);
      writeFileSync(layout.source, Buffer.from(`new-${label}`));

      await assert.rejects(
        publishNativeBinding(publicationOptions(layout, override)),
        /injected|verification/u,
      );
      assert.deepEqual(snapshotPair(layout), before);
      assertVerifiedPair(layout, before.binding);
    });
  }
});

test("publisher rejects an ambiguous partial destination before mutation", async (t) => {
  const partial = createLayout(t);
  mkdirSync(partial.destDir, { recursive: true });
  writeFileSync(partial.source, Buffer.from("replacement"));
  writeFileSync(partial.bindingPath, Buffer.from("orphan"));
  await assert.rejects(
    publishNativeBinding(publicationOptions(partial)),
    /partial binary\/checksum pair/u,
  );
  assert.deepEqual(readFileSync(partial.bindingPath), Buffer.from("orphan"));
  assert.equal(existsSync(partial.manifestPath), false);
  assert.deepEqual(publicationArtifacts(partial.destDir), []);
});

test("SIGKILL recovery resolves every durable replacement phase to an exact old or new pair", async (t) => {
  const phases = [
    ["staged-native-synced", "previous"],
    ["staged-manifest-synced", "previous"],
    ["journal-prepared", "previous"],
    ["previous-manifest-renamed", "previous"],
    ["previous-manifest-moved", "previous"],
    ["previous-native-renamed", "previous"],
    ["previous-native-moved", "previous"],
    ["next-native-renamed", "previous"],
    ["next-native-moved", "previous"],
    ["next-manifest-renamed", "next"],
    ["next-manifest-moved", "next"],
    ["published-pair-verified", "next"],
    ["published-verified", "next"],
    ["journal-committed", "next"],
  ];
  for (const [phase, expected] of phases) {
    await t.test(phase, async (subtest) => {
      const layout = createLayout(subtest);
      const previous = Buffer.from(`previous-${phase}`);
      const next = Buffer.from(`next-${phase}`);
      writeFileSync(layout.source, previous);
      await publishNativeBinding(publicationOptions(layout));
      writeFileSync(layout.source, next);

      await killPublisherAtPhase(layout, phase);
      const recovery = await recoverNativeBindingPublication({
        destDir: layout.destDir,
        platform: PLATFORM,
        arch: ARCH,
      });
      if (expected === "previous") {
        assert.ok(["existing", "previous"].includes(recovery.outcome));
      } else {
        assert.equal(recovery.outcome, expected);
      }
      assertVerifiedPair(layout, expected === "next" ? next : previous);

      const final = Buffer.from(`repeatable-${phase}`);
      writeFileSync(layout.source, final);
      await publishNativeBinding(publicationOptions(layout));
      assertVerifiedPair(layout, final);
    });
  }
});

test("SIGKILL recovery covers every first-publication phase without inventing a prior pair", async (t) => {
  const phases = [
    ["staged-native-synced", "absent"],
    ["staged-manifest-synced", "absent"],
    ["journal-prepared", "absent"],
    ["next-native-renamed", "absent"],
    ["next-native-moved", "absent"],
    ["next-manifest-renamed", "next"],
    ["next-manifest-moved", "next"],
    ["published-pair-verified", "next"],
    ["published-verified", "next"],
    ["journal-committed", "next"],
  ];
  for (const [phase, expected] of phases) {
    await t.test(phase, async (subtest) => {
      const layout = createLayout(subtest);
      const next = Buffer.from(`first-${phase}`);
      writeFileSync(layout.source, next);
      await killPublisherAtPhase(layout, phase);

      const recovery = await recoverNativeBindingPublication({
        destDir: layout.destDir,
        platform: PLATFORM,
        arch: ARCH,
      });
      if (expected === "next") {
        assert.equal(recovery.outcome, "next");
        assertVerifiedPair(layout, next);
      } else {
        assert.ok(["absent", "none"].includes(recovery.outcome));
        assert.equal(existsSync(layout.bindingPath), false);
        assert.equal(existsSync(layout.manifestPath), false);
        assert.deepEqual(publicationArtifacts(layout.destDir), []);
      }

      const final = Buffer.from(`repeatable-first-${phase}`);
      writeFileSync(layout.source, final);
      await publishNativeBinding(publicationOptions(layout));
      assertVerifiedPair(layout, final);
    });
  }
});

test("recovery refuses tampered backup bytes and preserves the forensic transaction", async (t) => {
  const layout = createLayout(t);
  const previous = Buffer.from("tamper-test-previous");
  writeFileSync(layout.source, previous);
  await publishNativeBinding(publicationOptions(layout));
  writeFileSync(layout.source, Buffer.from("tamper-test-next"));
  await killPublisherAtPhase(layout, "previous-native-moved");

  const transaction = readdirSync(layout.destDir).find((name) =>
    name.startsWith(".iroha-js-host-txn-"),
  );
  assert.ok(transaction);
  const backup = path.join(layout.destDir, transaction, `${NATIVE_FILENAME}.previous`);
  chmodSync(backup, 0o600);
  writeFileSync(backup, Buffer.from("attacker-replaced-backup"));

  await assert.rejects(
    recoverNativeBindingPublication({
      destDir: layout.destDir,
      platform: PLATFORM,
      arch: ARCH,
    }),
    /tampered|does not match the journal/u,
  );
  assert.equal(existsSync(layout.bindingPath), false);
  assert.equal(existsSync(layout.manifestPath), false);
  assert.ok(publicationArtifacts(layout.destDir).includes(transaction));
});

test("recovery rejects a journal with injected fields before mutating the prior pair", async (t) => {
  const layout = createLayout(t);
  const previous = Buffer.from("journal-tamper-previous");
  writeFileSync(layout.source, previous);
  await publishNativeBinding(publicationOptions(layout));
  writeFileSync(layout.source, Buffer.from("journal-tamper-next"));
  await killPublisherAtPhase(layout, "journal-prepared");

  const transaction = readdirSync(layout.destDir).find((name) =>
    name.startsWith(".iroha-js-host-txn-"),
  );
  assert.ok(transaction);
  const journal = path.join(layout.destDir, transaction, "journal-000000.json");
  const malicious = JSON.parse(readFileSync(journal, "utf8"));
  malicious.attacker = "injected";
  writeFileSync(journal, `${JSON.stringify(malicious)}\n`);

  await assert.rejects(
    recoverNativeBindingPublication({
      destDir: layout.destDir,
      platform: PLATFORM,
      arch: ARCH,
    }),
    /unexpected or missing fields/u,
  );
  assert.deepEqual(readFileSync(layout.bindingPath), previous);
  assert.equal(
    verifyNativeBinding(layout.bindingPath, {
      manifestPath: layout.manifestPath,
      platformKey: PLATFORM_KEY,
    }).ok,
    true,
  );
  assert.ok(publicationArtifacts(layout.destDir).includes(transaction));
});

test("recovery rejects a journal that skips durable phases", async (t) => {
  const layout = createLayout(t);
  const previous = Buffer.from("journal-phase-previous");
  writeFileSync(layout.source, previous);
  await publishNativeBinding(publicationOptions(layout));
  writeFileSync(layout.source, Buffer.from("journal-phase-next"));
  await killPublisherAtPhase(layout, "journal-prepared");

  const transaction = readdirSync(layout.destDir).find((name) =>
    name.startsWith(".iroha-js-host-txn-"),
  );
  assert.ok(transaction);
  const journal = path.join(layout.destDir, transaction, "journal-000000.json");
  const malicious = JSON.parse(readFileSync(journal, "utf8"));
  malicious.phase = "committed";
  writeFileSync(journal, `${JSON.stringify(malicious)}\n`);

  await assert.rejects(
    recoverNativeBindingPublication({
      destDir: layout.destDir,
      platform: PLATFORM,
      arch: ARCH,
    }),
    /journal phases are non-canonical/u,
  );
  assert.deepEqual(readFileSync(layout.bindingPath), previous);
  assert.ok(publicationArtifacts(layout.destDir).includes(transaction));
});

test("recovery bounds a maliciously oversized journal before parsing", async (t) => {
  const layout = createLayout(t);
  const previous = Buffer.from("oversized-journal-previous");
  writeFileSync(layout.source, previous);
  await publishNativeBinding(publicationOptions(layout));
  writeFileSync(layout.source, Buffer.from("oversized-journal-next"));
  await killPublisherAtPhase(layout, "journal-prepared");

  const transaction = readdirSync(layout.destDir).find((name) =>
    name.startsWith(".iroha-js-host-txn-"),
  );
  assert.ok(transaction);
  const journal = path.join(layout.destDir, transaction, "journal-000000.json");
  writeFileSync(journal, "[".repeat(16 * 1024 + 1));

  await assert.rejects(
    recoverNativeBindingPublication({
      destDir: layout.destDir,
      platform: PLATFORM,
      arch: ARCH,
    }),
    /journal entry exceeds 16384 bytes/u,
  );
  assert.deepEqual(readFileSync(layout.bindingPath), previous);
  assert.ok(publicationArtifacts(layout.destDir).includes(transaction));
});

test("recovery rejects duplicated exact old components instead of guessing ownership", async (t) => {
  const layout = createLayout(t);
  writeFileSync(layout.source, Buffer.from("duplicate-old-previous"));
  await publishNativeBinding(publicationOptions(layout));
  writeFileSync(layout.source, Buffer.from("duplicate-old-next"));
  await killPublisherAtPhase(layout, "journal-prepared");

  const transaction = readdirSync(layout.destDir).find((name) =>
    name.startsWith(".iroha-js-host-txn-"),
  );
  assert.ok(transaction);
  copyFileSync(
    layout.bindingPath,
    path.join(layout.destDir, transaction, `${NATIVE_FILENAME}.previous`),
  );
  await assert.rejects(
    recoverNativeBindingPublication({
      destDir: layout.destDir,
      platform: PLATFORM,
      arch: ARCH,
    }),
    /duplicate previous-pair components/u,
  );
  assert.deepEqual(readFileSync(layout.bindingPath), Buffer.from("duplicate-old-previous"));
});

test("recovery rejects a committed-looking pair when its exact prior backup is missing", async (t) => {
  const layout = createLayout(t);
  const next = Buffer.from("missing-prior-next");
  writeFileSync(layout.source, Buffer.from("missing-prior-previous"));
  await publishNativeBinding(publicationOptions(layout));
  writeFileSync(layout.source, next);
  await killPublisherAtPhase(layout, "next-manifest-renamed");

  const transaction = readdirSync(layout.destDir).find((name) =>
    name.startsWith(".iroha-js-host-txn-"),
  );
  assert.ok(transaction);
  rmSync(path.join(layout.destDir, transaction, `${NATIVE_FILENAME}.previous`));
  await assert.rejects(
    recoverNativeBindingPublication({
      destDir: layout.destDir,
      platform: PLATFORM,
      arch: ARCH,
    }),
    /missing or duplicate previous-pair components/u,
  );
  assert.deepEqual(readFileSync(layout.bindingPath), next);
  assert.equal(
    verifyNativeBinding(layout.bindingPath, {
      manifestPath: layout.manifestPath,
      platformKey: PLATFORM_KEY,
    }).ok,
    true,
  );
});

test("a partial next journal write is ignored while the last fsynced entry drives recovery", async (t) => {
  const layout = createLayout(t);
  const previous = Buffer.from("partial-journal-previous");
  writeFileSync(layout.source, previous);
  await publishNativeBinding(publicationOptions(layout));
  writeFileSync(layout.source, Buffer.from("partial-journal-next"));
  await killPublisherAtPhase(layout, "journal-prepared");

  const transaction = readdirSync(layout.destDir).find((name) =>
    name.startsWith(".iroha-js-host-txn-"),
  );
  assert.ok(transaction);
  writeFileSync(
    path.join(
      layout.destDir,
      transaction,
      ".journal-000001-00000000-0000-4000-8000-000000000000.tmp",
    ),
    '{"version":',
  );
  const recovery = await recoverNativeBindingPublication({
    destDir: layout.destDir,
    platform: PLATFORM,
    arch: ARCH,
  });
  assert.equal(recovery.outcome, "previous");
  assertVerifiedPair(layout, previous);
});

test("publisher refuses a checksum-invalid existing pair before creating a transaction", async (t) => {
  const layout = createLayout(t);
  writeFileSync(layout.source, Buffer.from("valid-existing"));
  await publishNativeBinding(publicationOptions(layout));
  chmodSync(layout.bindingPath, 0o600);
  writeFileSync(layout.bindingPath, Buffer.from("tampered-existing"));
  writeFileSync(layout.source, Buffer.from("replacement"));

  await assert.rejects(
    publishNativeBinding(publicationOptions(layout)),
    /Existing native binding(?: after recovery)? failed checksum verification/u,
  );
  assert.deepEqual(readFileSync(layout.bindingPath), Buffer.from("tampered-existing"));
  assert.deepEqual(publicationArtifacts(layout.destDir), []);
});

test("required-export probe accepts a complete module and rejects missing symbols", (t) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-probe-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const complete = path.join(root, "complete.cjs");
  const incomplete = path.join(root, "incomplete.cjs");
  writeFileSync(
    complete,
    `module.exports = { ${REQUIRED_NATIVE_EXPORTS.map((name) => `${name}() {}`).join(", ")} };\n`,
  );
  writeFileSync(incomplete, "module.exports = { noritoEncodeInstruction() {} };\n");

  assert.doesNotThrow(() => probeNativeBindingExports(complete));
  assert.throws(
    () => probeNativeBindingExports(incomplete),
    /missing required native exports.*noritoDecodeInstruction.*compileKotodama/u,
  );
  assert.throws(
    () => probeNativeBindingExports(complete, ["not-valid!"]),
    /non-empty identifier array/u,
  );
});

test(
  "required-export probe loads a real addon through the recovery-compatible private suffix",
  { skip: !existsSync(path.resolve("native", NATIVE_FILENAME)) },
  (t) => {
    const root = mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-dlopen-probe-"));
    t.after(() => rmSync(root, { recursive: true, force: true }));
    const staged = path.join(root, `${NATIVE_FILENAME}.next`);
    copyFileSync(path.resolve("native", NATIVE_FILENAME), staged);
    // This test isolates the loader path from the release export contract so a
    // locally cached addon from an older compatible build cannot make the
    // focused unit suite ambient-state dependent.
    assert.doesNotThrow(() =>
      probeNativeBindingExports(staged, ["noritoEncodeInstruction"]),
    );
  },
);
