// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  validateDefaultComposeGenesisCustody,
  validateQualificationEnvironment,
} from "../scripts/run_integration.mjs";

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

test("release qualification preflight requires the complete live SoraFS context", () => {
  assert.throws(
    () => validateQualificationEnvironment({}),
    /release qualification requires explicit live inputs/u,
  );

  const complete = {
    IROHA_TORII_INTEGRATION_URL: "https://torii.example.invalid",
    IROHA_TORII_INTEGRATION_MUTATE: "1",
    IROHA_TORII_INTEGRATION_SORAFS_ENABLED: "1",
    IROHA_TORII_INTEGRATION_SORAFS_FETCH_MANIFEST: "01".repeat(32),
    IROHA_TORII_INTEGRATION_SORAFS_FETCH_LENGTH: "4096",
    IROHA_TORII_INTEGRATION_SORAFS_POR_WEEK: "2026-W31",
    IROHA_TORII_INTEGRATION_UAID: `uaid:${"03".repeat(32)}`,
    IROHA_TORII_INTEGRATION_UAID_DATASPACE: "7",
    IROHA_TORII_INTEGRATION_SPACE_DIRECTORY_ENABLED: "1",
    IROHA_TORII_INTEGRATION_SPACE_DIRECTORY_MANIFEST: "/runtime/manifest.json",
    IROHA_TORII_INTEGRATION_SPACE_DIRECTORY_REVOKE_EPOCH: "11",
    IROHA_TORII_INTEGRATION_DA_ENABLED: "1",
    IROHA_TORII_INTEGRATION_DA_TICKET: "05".repeat(32),
    IROHA_TORII_INTEGRATION_DA_GATEWAYS: JSON.stringify([
      { name: "gateway-a" },
      { name: "gateway-b" },
    ]),
  };
  assert.doesNotThrow(() => validateQualificationEnvironment(complete));

  assert.throws(
    () =>
      validateQualificationEnvironment({
        ...complete,
        IROHA_TORII_INTEGRATION_UAID_DATASPACE: "",
      }),
    /IROHA_TORII_INTEGRATION_UAID_DATASPACE/u,
  );
  assert.throws(
    () =>
      validateQualificationEnvironment({
        ...complete,
        IROHA_TORII_INTEGRATION_DA_GATEWAYS: JSON.stringify([
          { name: "gateway-a" },
        ]),
      }),
    /at least two gateways/u,
  );
});
