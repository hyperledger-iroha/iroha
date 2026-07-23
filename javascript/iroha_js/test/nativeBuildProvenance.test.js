import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  createNativeBuildProvenance,
  nativeBuildProvenancePath,
  readNativeBuildProvenance,
  readNativeBuildSourceState,
  validateNativeBuildProvenance,
  writeNativeBuildProvenance,
} from "../scripts/native-build-provenance.mjs";

const REVISION = "a".repeat(40);

function withNativeFixture(run) {
  const directory = mkdtempSync(path.join(os.tmpdir(), "iroha-js-build-provenance-"));
  const nativePath = path.join(directory, "libiroha_js_host.so");
  writeFileSync(nativePath, "compiled-native-fixture");
  try {
    return run({ directory, nativePath });
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test("native build provenance binds the exact binary, revision, profile, and cleanliness", () => {
  withNativeFixture(({ nativePath }) => {
    const provenance = createNativeBuildProvenance({
      cargoProfile: "deploy",
      nativePath,
      sourceBefore: { sourceGitRevision: REVISION, sourceTreeClean: true },
      sourceAfter: { sourceGitRevision: REVISION, sourceTreeClean: true },
    });
    assert.equal(provenance.version, 1);
    assert.equal(provenance.cargo_profile, "deploy");
    assert.match(provenance.native_sha256, /^[0-9a-f]{64}$/u);
    assert.equal(provenance.source_git_revision, REVISION);
    assert.equal(provenance.source_tree_clean, true);

    const written = writeNativeBuildProvenance(nativePath, provenance);
    assert.equal(written, nativeBuildProvenancePath(nativePath));
    assert.deepEqual(readNativeBuildProvenance(nativePath), provenance);
    assert.match(readFileSync(written, "utf8"), /"source_tree_clean": true/u);
  });
});

test("a dirty source observation remains dirty and revision changes abort publication", () => {
  withNativeFixture(({ nativePath }) => {
    const dirty = createNativeBuildProvenance({
      cargoProfile: "debug",
      nativePath,
      sourceBefore: { sourceGitRevision: REVISION, sourceTreeClean: false },
      sourceAfter: { sourceGitRevision: REVISION, sourceTreeClean: true },
    });
    assert.equal(dirty.source_tree_clean, false);
    assert.throws(
      () =>
        createNativeBuildProvenance({
          cargoProfile: "debug",
          nativePath,
          sourceBefore: { sourceGitRevision: REVISION, sourceTreeClean: true },
          sourceAfter: {
            sourceGitRevision: "b".repeat(40),
            sourceTreeClean: true,
          },
        }),
      /revision changed/u,
    );
  });
});

test("native build provenance rejects a stale binary and unexpected fields", () => {
  withNativeFixture(({ nativePath }) => {
    const provenance = createNativeBuildProvenance({
      cargoProfile: "release",
      nativePath,
      sourceBefore: { sourceGitRevision: REVISION, sourceTreeClean: true },
      sourceAfter: { sourceGitRevision: REVISION, sourceTreeClean: true },
    });
    writeFileSync(nativePath, "different-native-output");
    assert.throws(
      () => validateNativeBuildProvenance(provenance, nativePath),
      /does not match/u,
    );
    assert.throws(
      () =>
        validateNativeBuildProvenance(
          { ...provenance, unexpected: true },
          nativePath,
        ),
      /unexpected or missing fields/u,
    );
  });
});

test("Git source state requires one exact revision and includes untracked files", () => {
  const calls = [];
  const state = readNativeBuildSourceState("/repo", {
    run(command, args) {
      calls.push([command, args]);
      return args.includes("rev-parse")
        ? { status: 0, stdout: `${REVISION}\n` }
        : { status: 0, stdout: "?? untracked-source\n" };
    },
  });
  assert.deepEqual(state, {
    sourceGitRevision: REVISION,
    sourceTreeClean: false,
  });
  assert.ok(calls[1][1].includes("--untracked-files=all"));
});
