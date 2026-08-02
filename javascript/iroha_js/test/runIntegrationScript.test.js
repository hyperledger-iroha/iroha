// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  composeUpArgs,
  validateDefaultComposeGenesisCustody,
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

test("default Compose custody preflight fails closed and accepts exact records", async (t) => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "iroha-js-compose-custody-"));
  t.after(() => rm(directory, { recursive: true, force: true }));

  await assert.rejects(
    validateDefaultComposeGenesisCustody({}),
    /IROHA_GENESIS_PUBLIC_KEY_FILE is required/,
  );

  const publicPath = path.join(directory, "public.key");
  const privatePath = path.join(directory, "private.key");
  await writeFile(publicPath, "public-without-newline");
  await writeFile(privatePath, "private\n");
  const env = {
    IROHA_GENESIS_PUBLIC_KEY_FILE: publicPath,
    IROHA_GENESIS_PRIVATE_KEY_FILE: privatePath,
  };
  await assert.rejects(
    validateDefaultComposeGenesisCustody(env),
    /exactly one non-empty key record/,
  );

  await writeFile(publicPath, "public\n");
  await validateDefaultComposeGenesisCustody(env);
});
