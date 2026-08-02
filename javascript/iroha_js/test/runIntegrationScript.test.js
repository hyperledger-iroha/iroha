// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  composeUpArgs,
  validateDefaultComposeGenesisArtifacts,
} from "../scripts/run_integration.mjs";

test("default Compose startup does not narrow the validator stack", () => {
  assert.deepEqual(composeUpArgs("defaults/docker-compose.single.yml"), [
    "-f",
    "defaults/docker-compose.single.yml",
    "up",
    "-d",
  ]);
  assert.deepEqual(composeUpArgs("custom.yml", "validator-a"), [
    "-f",
    "custom.yml",
    "up",
    "-d",
    "validator-a",
  ]);
});

test("default Compose artifact preflight fails closed and accepts exact records", async (t) => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "iroha-js-compose-custody-"));
  t.after(() => rm(directory, { recursive: true, force: true }));

  await assert.rejects(
    validateDefaultComposeGenesisArtifacts({}),
    /IROHA_GENESIS_PUBLIC_KEY_FILE is required/,
  );

  const publicPath = path.join(directory, "public.key");
  const signedPath = path.join(directory, "genesis.signed.nrt");
  const hashPath = path.join(directory, "genesis.expected_hash");
  await writeFile(publicPath, "public-without-newline");
  await writeFile(signedPath, "signed-genesis");
  await writeFile(hashPath, `${"0".repeat(63)}1\n`);
  const env = {
    IROHA_GENESIS_PUBLIC_KEY_FILE: publicPath,
    IROHA_GENESIS_SIGNED_FILE: signedPath,
    IROHA_GENESIS_EXPECTED_HASH_FILE: hashPath,
  };
  await assert.rejects(
    validateDefaultComposeGenesisArtifacts(env),
    /exactly one non-empty record/,
  );

  await writeFile(publicPath, "public\n");
  await writeFile(hashPath, `${"0".repeat(63)}2\n`);
  await assert.rejects(
    validateDefaultComposeGenesisArtifacts(env),
    /canonical lowercase Iroha hash/,
  );

  await writeFile(hashPath, `${"0".repeat(63)}1\n`);
  await validateDefaultComposeGenesisArtifacts(env);
});
