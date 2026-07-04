// Purpose: exercise the Solana -> TAIRA XOR SCCP deployment helper contract.
// Prerequisites: Node.js 18+ and no network access; fixtures are local temp files.

import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT = fileURLToPath(
  new URL("./sccp_solana_taira_xor_deploy.mjs", import.meta.url),
);
const HEX64 = "11".repeat(32);

const hex32 = (seed) => seed.toString(16).padStart(2, "0").repeat(32);

const browserProver = (seed) => ({
  module_url: `https://example.com/sccp-solana-prover-${seed}.mjs`,
  module_hash: hex32(seed),
  manifest_hash: hex32(seed + 1),
  bound_route_hash: hex32(seed + 2),
  bound_proof_hash: hex32(seed + 3),
  expected_exports: ["prove"],
});

const routeTemplate = (readiness) => ({
  production_ready: true,
  taira_xor_token_address: "SolanaXorTokenMint1111111111111111111111111",
  sccp_tron_source_bridge_address: "SolanaSourceBridge1111111111111111111111",
  tron_verifier_address: "SolanaVerifier11111111111111111111111111",
  destination_binding_key: "solana:taira_sol_xor:xor",
  verifier_code_hash: HEX64,
  verifier_key_hash: hex32(0x21),
  destination_binding_hash: hex32(0x22),
  taira_burn_record_settlement_asset_definition_id: "xor#taira",
  taira_burn_record_contract_artifact_b64: "AQID",
  taira_burn_record_artifact_sha256: hex32(0x23),
  taira_burn_record_code_hash: hex32(0x24),
  taira_burn_record_vk_backend: "halo2/ipa",
  taira_burn_record_vk_name: "taira-sol-xor-burn-record-v1",
  destination_browser_prover: browserProver(0x31),
  source_browser_prover: browserProver(0x41),
  ...readiness,
});

const routeEvidence = () => ({
  schema: "sccp-solana-taira-xor-program-evidence/v1",
  programId: "SolanaBridgeProgram1111111111111111111111111",
  programDataAddress: "SolanaProgramData111111111111111111111111111",
  programDataSlot: 123456,
  programAccountDataSha256: hex32(0x51),
});

const writeJson = async (file, value) => {
  await writeFile(file, `${JSON.stringify(value, null, 2)}\n`);
};

const runRouteManifest = (template, evidence, output) =>
  spawnSync(
    process.execPath,
    [
      SCRIPT,
      "route-manifest",
      "--template",
      template,
      "--evidence",
      evidence,
      "--output",
      output,
    ],
    {
      encoding: "utf8",
    },
  );

const routeFixture = async (name) => {
  const root = await mkdtemp(join(tmpdir(), `iroha-sccp-solana-${name}-`));
  const template = join(root, "template.json");
  const evidence = join(root, "evidence.json");
  const output = join(root, "route.manifest.json");
  await writeJson(evidence, routeEvidence());
  return { template, evidence, output };
};

test("Solana route-manifest accepts only literal production_ready true", async () => {
  const fixture = await routeFixture("ready");
  await writeJson(fixture.template, routeTemplate());

  const result = runRouteManifest(
    fixture.template,
    fixture.evidence,
    fixture.output,
  );

  assert.equal(result.status, 0, result.stderr);
  const manifest = JSON.parse(await readFile(fixture.output, "utf8"));
  assert.equal(manifest.production_ready, true);
  assert.equal(manifest.route_id, "taira_sol_xor");
});

test("Solana route-manifest rejects malformed production_ready values without writing", async () => {
  const cases = [
    ["string-false", { production_ready: "false" }],
    ["string-true", { production_ready: "true" }],
    ["number-one", { production_ready: 1 }],
    ["boolean-false", { production_ready: false }],
    ["missing", { production_ready: undefined }],
    ["camel-string-false", { production_ready: undefined, productionReady: "false" }],
    ["duplicate-alias", { production_ready: true, productionReady: true }],
  ];

  for (const [name, readiness] of cases) {
    const fixture = await routeFixture(name);
    const template = routeTemplate(readiness);
    if (readiness.production_ready === undefined) {
      delete template.production_ready;
    }
    await writeJson(fixture.template, template);

    const result = runRouteManifest(
      fixture.template,
      fixture.evidence,
      fixture.output,
    );

    assert.notEqual(result.status, 0, `${name} unexpectedly succeeded`);
    assert.match(
      result.stderr,
      /production_ready must (?:be the boolean true|not use multiple aliases)/u,
      name,
    );
    assert.equal(existsSync(fixture.output), false, `${name} wrote output`);
  }
});

test("Solana route-manifest does not overwrite output on malformed production_ready", async () => {
  const fixture = await routeFixture("overwrite");
  const sentinel = "sentinel:existing-manifest\n";
  await writeFile(fixture.output, sentinel);
  await writeJson(
    fixture.template,
    routeTemplate({ production_ready: "false" }),
  );

  const result = runRouteManifest(
    fixture.template,
    fixture.evidence,
    fixture.output,
  );

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /production_ready must be the boolean true/u);
  assert.equal(await readFile(fixture.output, "utf8"), sentinel);
});
