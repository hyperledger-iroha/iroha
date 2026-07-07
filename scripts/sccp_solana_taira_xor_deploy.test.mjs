#!/usr/bin/env node
// Unit tests for the TAIRA XOR Solana deployment helper's offline manifest
// validation. These tests do not contact Solana or TAIRA.
import assert from "node:assert/strict";
import {
  lstat,
  mkdtemp,
  readFile,
  rm,
  stat,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import {
  main,
  normalizeManifest,
} from "./sccp_solana_taira_xor_deploy.mjs";

const hex32 = (byte) => `${byte.repeat(32)}`;

const browserProver = (byte) => ({
  module_url: `https://provers.sora.org/sccp-solana-${byte}.mjs`,
  module_hash: hex32(byte),
  manifest_hash: hex32("aa"),
  bound_route_hash: hex32("33"),
  bound_proof_hash: hex32("cc"),
  expected_exports: ["verifySccpProof"],
});

const validEvidence = () => ({
  schema: "sccp-solana-taira-xor-program-evidence/v1",
  programId: "Bridge11111111111111111111111111111111111111",
  programDataAddress: "ProgramData1111111111111111111111111111111111",
  programDataSlot: 12345,
  programAccountDataSha256: hex32("dd"),
});

const validTemplate = (overrides = {}) => ({
  production_ready: true,
  taira_xor_token_address: "TokenMint11111111111111111111111111111111111",
  taira_xor_bridge_address: "Bridge11111111111111111111111111111111111111",
  sccp_solana_source_bridge_address:
    "SourceBridge111111111111111111111111111111111",
  solana_verifier_program_id:
    "Verifier1111111111111111111111111111111111111",
  destination_binding_key: "sccp:solana-testnet:taira_sol_xor",
  taira_burn_record_settlement_asset_definition_id: "xor#sora",
  taira_burn_record_contract_artifact_b64: "AQIDBA==",
  taira_burn_record_vk_backend: "groth16",
  taira_burn_record_vk_name: "taira_sol_xor_burn_record_v1",
  verifier_code_hash: hex32("11"),
  verifier_key_hash: hex32("22"),
  destination_binding_hash: hex32("33"),
  taira_burn_record_artifact_sha256: hex32("44"),
  taira_burn_record_code_hash: hex32("55"),
  destination_browser_prover: browserProver("66"),
  source_browser_prover: browserProver("77"),
  ...overrides,
});

const withTempDir = async (fn) => {
  const dir = await mkdtemp(join(tmpdir(), "iroha-sccp-solana-route-"));
  try {
    return await fn(dir);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
};

const writeJson = (file, value) =>
  writeFile(file, `${JSON.stringify(value, null, 2)}\n`);

const retiredTopLevelAliases = [
  ["productionReady", "production_ready"],
  ["routeId", "route_id"],
  ["assetKey", "asset_key"],
  ["solanaNetwork", "solana_network"],
  ["chainIdHex", "chain_id_hex"],
  ["counterpartyAccountCodec", "counterparty_account_codec"],
  ["counterpartyAccountCodecKey", "counterparty_account_codec_key"],
  ["counterpartyDomain", "counterparty_domain"],
  ["verifierTarget", "verifier_target"],
  ["networkIdHex", "network_id_hex"],
  ["destinationBrowserProver", "destination_browser_prover"],
  ["sourceBrowserProver", "source_browser_prover"],
  ["sourceVerifierMaterial", "source_verifier_material"],
  ["sourceAdapterEngineDeployment", "source_adapter_engine_deployment"],
  ["sourceAdapterEngine", "source_adapter_engine"],
  ["taira_xor_solana_program_id", "taira_xor_bridge_address"],
  ["solana_program_id", "taira_xor_bridge_address"],
  ["tairaXorSolanaProgramId", "taira_xor_bridge_address"],
  ["solanaProgramId", "taira_xor_bridge_address"],
  ["solana_token_mint", "taira_xor_token_address"],
  ["tairaXorTokenAddress", "taira_xor_token_address"],
  ["solanaTokenMint", "taira_xor_token_address"],
  ["solana_source_bridge_address", "sccp_solana_source_bridge_address"],
  ["sccpSolanaSourceBridgeAddress", "sccp_solana_source_bridge_address"],
  ["solanaSourceBridgeAddress", "sccp_solana_source_bridge_address"],
  ["solanaVerifierProgramId", "solana_verifier_program_id"],
  [
    "sccp_solana_destination_verifier_program_id",
    "solana_verifier_program_id",
  ],
  ["sccpSolanaDestinationVerifierProgramId", "solana_verifier_program_id"],
];

const retiredBrowserProverAliases = [
  ["moduleSpecifier", "module_specifier"],
  ["moduleUrl", "module_url"],
  ["moduleHash", "module_hash"],
  ["manifestHash", "manifest_hash"],
  ["expectedExports", "expected_exports"],
  ["boundRouteHash", "bound_route_hash"],
  ["boundProofHash", "bound_proof_hash"],
];

test("Solana route-manifest accepts only literal production_ready true", () => {
  const manifest = normalizeManifest(validTemplate(), validEvidence());
  assert.equal(manifest.production_ready, true);
  assert.equal(
    Object.prototype.hasOwnProperty.call(manifest, "productionReady"),
    false,
  );
});

test("Solana route-manifest rejects retired camel productionReady alias", () => {
  const template = validTemplate({
    productionReady: "secret-token-solana-retired-production-ready",
  });
  delete template.production_ready;

  assert.throws(
    () => normalizeManifest(template, validEvidence()),
    (error) => {
      assert.match(
        error.message,
        /Solana route-manifest template must not use retired productionReady; use production_ready\./u,
      );
      assert.doesNotMatch(error.message, /secret-token-solana-retired-production-ready/u);
      return true;
    },
  );
});

test("Solana route-manifest rejects retired top-level aliases without echoing values", () => {
  for (const [field, replacement] of retiredTopLevelAliases) {
    assert.throws(
      () =>
        normalizeManifest(
          validTemplate({
            [field]: { secret: `secret-token-solana-retired-${field}` },
          }),
          validEvidence(),
        ),
      (error) => {
        assert.equal(
          error.message,
          `Solana route-manifest template must not use retired ${field}; use ${replacement}.`,
        );
        assert.doesNotMatch(error.message, /secret-token-solana-retired/u);
        return true;
      },
      `accepted retired ${field} alias`,
    );
  }
});

test("Solana route-manifest fills only absent canonical bridge address from evidence", () => {
  const withoutBridgeAddress = validTemplate();
  delete withoutBridgeAddress.taira_xor_bridge_address;
  assert.equal(
    normalizeManifest(withoutBridgeAddress, validEvidence())
      .taira_xor_bridge_address,
    validEvidence().programId,
  );

  assert.throws(
    () =>
      normalizeManifest(
        validTemplate({ taira_xor_bridge_address: "" }),
        validEvidence(),
      ),
    /Solana bridge program must be a non-empty string without surrounding whitespace/u,
  );
});

test("Solana route-manifest rejects malformed production_ready values", () => {
  for (const value of ["true", "false", " TRUE", "1", 1, 0, null, {}, []]) {
    assert.throws(
      () =>
        normalizeManifest(
          validTemplate({ production_ready: value }),
          validEvidence(),
        ),
      /production_ready must be the boolean true/u,
      `accepted malformed production_ready value ${String(value)}`,
    );
  }
});

test("Solana route-manifest rejects coerced scalar values", () => {
  const scalarCases = [
    [
      { taira_xor_token_address: 1 },
      /Solana XOR token mint must be a non-empty string without surrounding whitespace/u,
    ],
    [
      { destination_binding_key: " sccp:solana-testnet:taira_sol_xor" },
      /destination binding key must be a non-empty string without surrounding whitespace/u,
    ],
    [
      { verifier_code_hash: hex32("AA") },
      /verifier_code_hash must be a 32-byte lowercase hex value/u,
    ],
    [{ version: "1" }, /version must be an integer/u],
    [{ counterparty_domain: "3" }, /counterparty_domain must be an integer/u],
    [
      { taira_burn_record_gas_limit: "2000000" },
      /taira_burn_record_gas_limit must be an integer/u,
    ],
    [
      {
        destination_browser_prover: {
          ...browserProver("66"),
          expected_exports: [7],
        },
      },
      /destination_browser_prover expected_exports contains an invalid export/u,
    ],
  ];

  for (const [overrides, expected] of scalarCases) {
    assert.throws(
      () => normalizeManifest(validTemplate(overrides), validEvidence()),
      expected,
    );
  }
});

test("Solana route-manifest rejects unsafe browser prover module URLs", () => {
  const credentialedUrl =
    "https://operator:secret-token-solana-browser-module@provers.sora.org/mod.mjs";
  const cases = [
    [
      credentialedUrl,
      /destination_browser_prover\.module_url must not contain credentials, params, query strings, or fragments/u,
    ],
    [
      "https://provers.sora.org/mod.mjs?secret=secret-token-solana-browser-module",
      /destination_browser_prover\.module_url must not contain credentials, params, query strings, or fragments/u,
    ],
    [
      "https://provers.sora.org/mod.mjs#secret-token-solana-browser-module",
      /destination_browser_prover\.module_url must not contain credentials, params, query strings, or fragments/u,
    ],
    [
      "https://provers.sora.org/mod.mjs;param",
      /destination_browser_prover\.module_url must not contain credentials, params, query strings, or fragments/u,
    ],
    [
      "http://provers.sora.org/mod.mjs",
      /destination_browser_prover\.module_url must use HTTPS or loopback HTTP/u,
    ],
    [
      "ftp://provers.sora.org/mod.mjs",
      /destination_browser_prover\.module_url must use HTTPS or loopback HTTP/u,
    ],
    [
      "https://localhost/mod.mjs",
      /destination_browser_prover\.module_url HTTPS URLs must use public DNS/u,
    ],
    [
      "https://127.0.0.1/mod.mjs",
      /destination_browser_prover\.module_url HTTPS URLs must use public DNS/u,
    ],
    [
      "https://provers/mod.mjs",
      /destination_browser_prover\.module_url HTTPS URLs must use public DNS/u,
    ],
    [
      "https://provers.local/mod.mjs",
      /destination_browser_prover\.module_url HTTPS URLs must use public DNS/u,
    ],
    [
      "https://bad_host.sora.org/mod.mjs",
      /destination_browser_prover\.module_url HTTPS URLs must use public DNS/u,
    ],
    [
      "../mod.mjs",
      /destination_browser_prover\.module_url must not traverse parent directories/u,
    ],
    [
      "/mod.mjs",
      /destination_browser_prover\.module_url must be package-relative, HTTPS, or loopback HTTP/u,
    ],
    [
      ".//mod.mjs",
      /destination_browser_prover\.module_url must be package-relative, HTTPS, or loopback HTTP/u,
    ],
    [
      "./mod%2emjs",
      /destination_browser_prover\.module_url must be package-relative, HTTPS, or loopback HTTP/u,
    ],
    [
      " ./mod.mjs",
      /destination_browser_prover\.module_url must be a deterministic module URL/u,
    ],
  ];

  for (const [moduleUrl, expected] of cases) {
    assert.throws(
      () =>
        normalizeManifest(
          validTemplate({
            destination_browser_prover: {
              ...browserProver("66"),
              module_url: moduleUrl,
            },
          }),
          validEvidence(),
        ),
      expected,
    );
  }
});

test("Solana route-manifest accepts package-relative browser prover module URLs", () => {
  const manifest = normalizeManifest(
    validTemplate({
      destination_browser_prover: {
        ...browserProver("66"),
        module_url: "./sccp-solana-destination.mjs",
      },
      source_browser_prover: {
        ...browserProver("77"),
        module_url: "@sora/sccp-solana/source.mjs",
      },
    }),
    validEvidence(),
  );

  assert.equal(
    manifest.destination_browser_prover.module_url,
    "./sccp-solana-destination.mjs",
  );
  assert.equal(
    manifest.source_browser_prover.module_url,
    "@sora/sccp-solana/source.mjs",
  );
});

test("Solana route-manifest rejects browser prover alias and export drift", () => {
  const cases = [
    [
      {
        destination_browser_prover: {
          ...browserProver("66"),
          moduleUrl: "https://provers.sora.org/alias.mjs",
        },
      },
      /destination_browser_prover must not use retired moduleUrl; use module_url\./u,
    ],
    [
      {
        destination_browser_prover: {
          ...browserProver("66"),
          expected_exports: [],
        },
      },
      /destination_browser_prover expected_exports must be a non-empty array/u,
    ],
    [
      {
        destination_browser_prover: {
          ...browserProver("66"),
          expected_exports: ["verifySccpProof", "verifySccpProof"],
        },
      },
      /destination_browser_prover expected_exports must not contain duplicates/u,
    ],
    [
      {
        destination_browser_prover: {
          ...browserProver("66"),
          expected_exports: ["1badExport"],
        },
      },
      /destination_browser_prover expected_exports contains an invalid export/u,
    ],
    [
      {
        destination_browser_prover: {
          ...browserProver("66"),
          bound_route_hash: hex32("99"),
        },
      },
      /destination_browser_prover\.bound_route_hash must match destination_binding_hash/u,
    ],
    [
      {
        source_browser_prover: {
          ...browserProver("77"),
          bound_route_hash: hex32("99"),
        },
      },
      /source_browser_prover\.bound_route_hash must match destination_binding_hash/u,
    ],
  ];

  for (const [overrides, expected] of cases) {
    assert.throws(
      () => normalizeManifest(validTemplate(overrides), validEvidence()),
      expected,
    );
  }
});

test("Solana route-manifest rejects retired browser prover aliases without echoing values", () => {
  for (const [field, replacement] of retiredBrowserProverAliases) {
    assert.throws(
      () =>
        normalizeManifest(
          validTemplate({
            destination_browser_prover: {
              ...browserProver("66"),
              [field]: `secret-token-solana-retired-browser-${field}`,
            },
          }),
          validEvidence(),
        ),
      (error) => {
        assert.equal(
          error.message,
          `destination_browser_prover must not use retired ${field}; use ${replacement}.`,
        );
        assert.doesNotMatch(
          error.message,
          /secret-token-solana-retired-browser/u,
        );
        return true;
      },
      `accepted retired browser prover ${field} alias`,
    );
  }
});

test("Solana route-manifest rejects malformed optional source adapter objects without echoing values", () => {
  const cases = [
    [
      { source_verifier_material: "secret-token-solana-source-material" },
      /source_verifier_material must be an object/u,
    ],
    [
      { source_verifier_material: ["secret-token-solana-source-material"] },
      /source_verifier_material must be an object/u,
    ],
    [
      {
        source_adapter_engine_deployment:
          "secret-token-solana-source-deployment",
      },
      /source_adapter_engine_deployment must be an object/u,
    ],
    [
      { source_adapter_engine: "secret-token-solana-source-engine" },
      /source_adapter_engine must be an object/u,
    ],
  ];

  for (const [overrides, expected] of cases) {
    assert.throws(
      () => normalizeManifest(validTemplate(overrides), validEvidence()),
      (error) => {
        assert.match(error.message, expected);
        assert.doesNotMatch(error.message, /secret-token-solana-source/u);
        return true;
      },
    );
  }
});

test("Solana route-manifest rejects malformed ProgramData evidence without echoing values", () => {
  const cases = [
    [
      {
        programDataAddress: {
          secret: "secret-token-solana-programdata-address",
        },
      },
      /evidence programDataAddress must be a non-empty string without surrounding whitespace/u,
    ],
    [
      { programDataSlot: "secret-token-solana-programdata-slot" },
      /evidence programDataSlot must be an integer/u,
    ],
    [
      { programAccountDataSha256: `AA${"0".repeat(62)}` },
      /evidence programAccountDataSha256 must be a 32-byte lowercase hex value/u,
    ],
  ];

  for (const [evidenceOverrides, expected] of cases) {
    assert.throws(
      () =>
        normalizeManifest(validTemplate(), {
          ...validEvidence(),
          ...evidenceOverrides,
        }),
      (error) => {
        assert.match(error.message, expected);
        assert.doesNotMatch(error.message, /secret-token-solana-programdata/u);
        return true;
      },
    );
  }
});

test("Solana route-manifest rejects missing, false, and retired production flags", () => {
  const missing = validTemplate();
  delete missing.production_ready;
  assert.throws(
    () => normalizeManifest(missing, validEvidence()),
    /production_ready must be the boolean true/u,
  );
  assert.throws(
    () =>
      normalizeManifest(
        validTemplate({ production_ready: false }),
        validEvidence(),
      ),
    /production_ready must be the boolean true/u,
  );
  assert.throws(
    () =>
      normalizeManifest(
        validTemplate({ productionReady: true }),
        validEvidence(),
      ),
    /Solana route-manifest template must not use retired productionReady; use production_ready\./u,
  );
});

test("Solana route-manifest CLI rejects truthy production_ready before writing output", async () => {
  await withTempDir(async (dir) => {
    const templatePath = join(dir, "template.json");
    const evidencePath = join(dir, "evidence.json");
    const outputPath = join(dir, "manifest.json");
    await writeJson(templatePath, validTemplate({ production_ready: "true" }));
    await writeJson(evidencePath, validEvidence());

    await assert.rejects(
      () =>
        main([
          "route-manifest",
          "--template",
          templatePath,
          "--evidence",
          evidencePath,
          "--output",
          outputPath,
        ]),
      /production_ready must be the boolean true/u,
    );
    await assert.rejects(() => stat(outputPath), /ENOENT/u);
  });
});

test("Solana route-manifest CLI does not overwrite output on malformed production_ready", async () => {
  await withTempDir(async (dir) => {
    const templatePath = join(dir, "template.json");
    const evidencePath = join(dir, "evidence.json");
    const outputPath = join(dir, "manifest.json");
    const sentinel = "sentinel:existing-manifest\n";
    await writeJson(templatePath, validTemplate({ production_ready: "false" }));
    await writeJson(evidencePath, validEvidence());
    await writeFile(outputPath, sentinel);

    await assert.rejects(
      () =>
        main([
          "route-manifest",
          "--template",
          templatePath,
          "--evidence",
          evidencePath,
          "--output",
          outputPath,
        ]),
      /production_ready must be the boolean true/u,
    );
    assert.equal(await readFile(outputPath, "utf8"), sentinel);
  });
});

test("Solana route-manifest CLI rejects retired productionReady before writing output", async () => {
  await withTempDir(async (dir) => {
    const templatePath = join(dir, "template.json");
    const evidencePath = join(dir, "evidence.json");
    const outputPath = join(dir, "manifest.json");
    const template = validTemplate({
      productionReady: "secret-token-solana-retired-production-ready",
    });
    delete template.production_ready;
    await writeJson(templatePath, template);
    await writeJson(evidencePath, validEvidence());

    await assert.rejects(
      async () => {
        try {
          await main([
            "route-manifest",
            "--template",
            templatePath,
            "--evidence",
            evidencePath,
            "--output",
            outputPath,
          ]);
        } catch (error) {
          assert.doesNotMatch(
            error.message,
            /secret-token-solana-retired-production-ready/u,
          );
          throw error;
        }
      },
      /Solana route-manifest template must not use retired productionReady; use production_ready\./u,
    );
    await assert.rejects(() => stat(outputPath), /ENOENT/u);
  });
});

test("Solana route-manifest CLI rejects retired top-level aliases before writing output", async () => {
  await withTempDir(async (dir) => {
    const templatePath = join(dir, "template.json");
    const evidencePath = join(dir, "evidence.json");
    const outputPath = join(dir, "manifest.json");
    await writeJson(
      templatePath,
      validTemplate({ routeId: "secret-token-solana-retired-route-id" }),
    );
    await writeJson(evidencePath, validEvidence());

    await assert.rejects(
      async () => {
        try {
          await main([
            "route-manifest",
            "--template",
            templatePath,
            "--evidence",
            evidencePath,
            "--output",
            outputPath,
          ]);
        } catch (error) {
          assert.doesNotMatch(
            error.message,
            /secret-token-solana-retired-route-id/u,
          );
          throw error;
        }
      },
      /Solana route-manifest template must not use retired routeId; use route_id\./u,
    );
    await assert.rejects(() => stat(outputPath), /ENOENT/u);
  });
});

test("Solana route-manifest CLI rejects duplicate options before writing output", async () => {
  await withTempDir(async (dir) => {
    const templatePath = join(dir, "template.json");
    const evidencePath = join(dir, "evidence.json");
    const outputPath = join(dir, "manifest.json");
    await writeJson(templatePath, validTemplate());
    await writeJson(evidencePath, validEvidence());

    await assert.rejects(
      () =>
        main([
          "route-manifest",
          "--template",
          templatePath,
          "--template",
          templatePath,
          "--evidence",
          evidencePath,
          "--output",
          outputPath,
        ]),
      /Option must be specified at most once/u,
    );
    await assert.rejects(() => stat(outputPath), /ENOENT/u);
  });
});

test("Solana route-manifest CLI rejects unknown options without echoing names", async () => {
  await withTempDir(async (dir) => {
    const templatePath = join(dir, "template.json");
    const evidencePath = join(dir, "evidence.json");
    const outputPath = join(dir, "manifest.json");
    await writeJson(templatePath, validTemplate());
    await writeJson(evidencePath, validEvidence());

    await assert.rejects(
      async () => {
        try {
          await main([
            "route-manifest",
            "--template",
            templatePath,
            "--evidence",
            evidencePath,
            "--output",
            outputPath,
            "--secret-token-solana-route",
            "value",
          ]);
        } catch (error) {
          assert.doesNotMatch(error.message, /secret-token-solana-route/u);
          throw error;
        }
      },
      /Unknown option/u,
    );
    await assert.rejects(() => stat(outputPath), /ENOENT/u);
  });
});

test("Solana CLI rejects unknown commands without echoing values", async () => {
  await assert.rejects(
    async () => {
      try {
        await main(["secret-token-solana-command"]);
      } catch (error) {
        assert.doesNotMatch(error.message, /secret-token-solana-command/u);
        throw error;
      }
    },
    /Unknown command\./u,
  );
});

test("Solana route-manifest CLI rejects positional arguments without echoing values", async () => {
  await withTempDir(async (dir) => {
    const templatePath = join(dir, "template.json");
    const evidencePath = join(dir, "evidence.json");
    const outputPath = join(dir, "manifest.json");
    await writeJson(templatePath, validTemplate());
    await writeJson(evidencePath, validEvidence());

    await assert.rejects(
      async () => {
        try {
          await main([
            "route-manifest",
            "--template",
            templatePath,
            "secret-token-solana-positional",
            "--evidence",
            evidencePath,
            "--output",
            outputPath,
          ]);
        } catch (error) {
          assert.doesNotMatch(error.message, /secret-token-solana-positional/u);
          throw error;
        }
      },
      /Unexpected positional argument\./u,
    );
    await assert.rejects(() => stat(outputPath), /ENOENT/u);
  });
});

test("Solana deploy CLI rejects malformed final flag before spawning", async () => {
  await assert.rejects(
    () =>
      main([
        "deploy",
        "--program-so",
        "program.so",
        "--program-id-keypair",
        "program-keypair.json",
        "--keypair",
        "deployer-keypair.json",
        "--broadcast",
        "true",
        "--confirm-network",
        "taira_sol_xor:solana-testnet",
        "--final",
        "maybe",
      ]),
    /--final must be true or false/u,
  );
});

test("Solana deploy CLI requires canonical network confirmation before spawning", async () => {
  await assert.rejects(
    () =>
      main([
        "deploy",
        "--program-so",
        "program.so",
        "--program-id-keypair",
        "program-keypair.json",
        "--keypair",
        "deployer-keypair.json",
        "--broadcast",
        "true",
      ]),
    /deploy requires --broadcast true --confirm-network taira_sol_xor:solana-testnet/u,
  );
});

test("Solana deploy CLI rejects retired testnet confirmation before spawning", async () => {
  await assert.rejects(
    async () => {
      try {
        await main([
          "deploy",
          "--program-so",
          "program.so",
          "--program-id-keypair",
          "program-keypair.json",
          "--keypair",
          "deployer-keypair.json",
          "--broadcast",
          "true",
          "--confirm-testnet",
          "secret-token-solana-retired-confirmation",
        ]);
      } catch (error) {
        assert.doesNotMatch(error.message, /secret-token-solana-retired/u);
        throw error;
      }
    },
    /Unknown option/u,
  );
});

test("Solana evidence CLI rejects bare keypair before spawning", async () => {
  await withTempDir(async (dir) => {
    const outputPath = join(dir, "evidence.json");

    await assert.rejects(
      () =>
        main([
          "evidence",
          "--program-id",
          "Bridge11111111111111111111111111111111111111",
          "--output",
          outputPath,
          "--keypair",
        ]),
      /--keypair must be specified with an explicit value/u,
    );
    await assert.rejects(() => stat(outputPath), /ENOENT/u);
  });
});

test("Solana route-manifest CLI rejects padded valued options before writing output", async () => {
  await withTempDir(async (dir) => {
    const templatePath = join(dir, "template.json");
    const evidencePath = join(dir, "evidence.json");
    const outputPath = join(dir, "manifest.json");
    await writeJson(templatePath, validTemplate());
    await writeJson(evidencePath, validEvidence());

    await assert.rejects(
      () =>
        main([
          "route-manifest",
          "--template",
          ` ${templatePath}`,
          "--evidence",
          evidencePath,
          "--output",
          outputPath,
        ]),
      /--template must be a non-empty value without surrounding whitespace/u,
    );
    await assert.rejects(() => stat(outputPath), /ENOENT/u);

    await assert.rejects(
      () =>
        main([
          "route-manifest",
          "--template",
          templatePath,
          "--evidence",
          evidencePath,
          "--output",
          `${outputPath} `,
        ]),
      /--output must be a non-empty value without surrounding whitespace/u,
    );
    await assert.rejects(() => stat(outputPath), /ENOENT/u);
  });
});

test("Solana route-manifest CLI rejects output path collisions with inputs", async () => {
  await withTempDir(async (dir) => {
    const templatePath = join(dir, "template.json");
    const evidencePath = join(dir, "evidence.json");
    const templateText = `${JSON.stringify(validTemplate(), null, 2)}\n`;
    const evidenceText = `${JSON.stringify(validEvidence(), null, 2)}\n`;
    await writeFile(templatePath, templateText);
    await writeFile(evidencePath, evidenceText);

    await assert.rejects(
      () =>
        main([
          "route-manifest",
          "--template",
          templatePath,
          "--evidence",
          evidencePath,
          "--output",
          templatePath,
        ]),
      /--output must not be the same path as --template/u,
    );
    assert.equal(await readFile(templatePath, "utf8"), templateText);

    await assert.rejects(
      () =>
        main([
          "route-manifest",
          "--template",
          templatePath,
          "--evidence",
          evidencePath,
          "--output",
          evidencePath,
        ]),
      /--output must not be the same path as --evidence/u,
    );
    assert.equal(await readFile(evidencePath, "utf8"), evidenceText);

    const linkedInputDir = join(dir, "linked-inputs");
    await symlink(dir, linkedInputDir);

    await assert.rejects(
      () =>
        main([
          "route-manifest",
          "--template",
          templatePath,
          "--evidence",
          evidencePath,
          "--output",
          join(linkedInputDir, "template.json"),
        ]),
      /--output must not be the same path as --template/u,
    );
    assert.equal(await readFile(templatePath, "utf8"), templateText);

    await assert.rejects(
      () =>
        main([
          "route-manifest",
          "--template",
          templatePath,
          "--evidence",
          evidencePath,
          "--output",
          join(linkedInputDir, "evidence.json"),
        ]),
      /--output must not be the same path as --evidence/u,
    );
    assert.equal(await readFile(evidencePath, "utf8"), evidenceText);
  });
});

test("Solana doctor rejects unsafe Solana RPC URLs before network", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async () => {
    throw new Error("fetch should not be called for unsafe Solana RPC URL");
  };
  try {
    const credentialedUrl =
      "https://operator:secret-token-solana-rpc-url@api.testnet.solana.com";
    const cases = [
      {
        value: "http://api.testnet.solana.com",
        expected:
          /--solana-rpc-url must use HTTPS unless it is loopback HTTP/u,
      },
      {
        value: credentialedUrl,
        expected:
          /--solana-rpc-url must not include credentials, params, query strings, or fragments/u,
      },
      {
        value:
          "https://api.testnet.solana.com?api_key=secret-token-solana-rpc-url",
        expected:
          /--solana-rpc-url must not include credentials, params, query strings, or fragments/u,
      },
      {
        value: "https://api.testnet.solana.com#secret-token-solana-rpc-url",
        expected:
          /--solana-rpc-url must not include credentials, params, query strings, or fragments/u,
      },
      {
        value: "https://api.testnet.solana.com/root;param",
        expected:
          /--solana-rpc-url must not include credentials, params, query strings, or fragments/u,
      },
      {
        value: "ftp://api.testnet.solana.com",
        expected:
          /--solana-rpc-url must use HTTPS unless it is loopback HTTP/u,
      },
      {
        value: " https://api.testnet.solana.com",
        expected: /--solana-rpc-url must be a valid HTTP\(S\) URL/u,
      },
      {
        value: "https://api.testnet.solana.com\n",
        expected: /--solana-rpc-url must be a valid HTTP\(S\) URL/u,
      },
      {
        value: "https://localhost",
        expected: /--solana-rpc-url HTTPS host must use public DNS/u,
      },
      {
        value: "https://127.0.0.1",
        expected: /--solana-rpc-url HTTPS host must use public DNS/u,
      },
      {
        value: "https://solana",
        expected: /--solana-rpc-url HTTPS host must use public DNS/u,
      },
      {
        value: "https://solana.local",
        expected: /--solana-rpc-url HTTPS host must use public DNS/u,
      },
      {
        value: "https://bad_host.solana.com",
        expected: /--solana-rpc-url HTTPS host must use public DNS/u,
      },
      {
        value: "not a url",
        expected: /--solana-rpc-url must be a valid HTTP\(S\) URL/u,
      },
    ];

    for (const testCase of cases) {
      await assert.rejects(
        async () => {
          try {
            await main(["doctor", "--solana-rpc-url", testCase.value]);
          } catch (error) {
            assert.doesNotMatch(error.message, /secret-token-solana-rpc-url/u);
            throw error;
          }
        },
        testCase.expected,
      );
    }
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("Solana helper ignores environment URL defaults", async () => {
  const previousSolanaRpc = process.env.SCCP_SOLANA_TESTNET_RPC_URL;
  const previousFallbackRpc = process.env.SOLANA_RPC_URL;
  const previousTorii = process.env.SCCP_TAIRA_TORII_URL;
  const originalFetch = globalThis.fetch;
  const originalLog = console.log;
  const calls = [];

  process.env.SCCP_SOLANA_TESTNET_RPC_URL =
    "https://operator:secret-token-solana-env-rpc@api.testnet.solana.com";
  process.env.SOLANA_RPC_URL =
    "https://operator:secret-token-solana-env-fallback@api.testnet.solana.com";
  process.env.SCCP_TAIRA_TORII_URL =
    "https://operator:secret-token-solana-env-torii@taira.sora.org";

  globalThis.fetch = async (url, options = {}) => {
    calls.push({ url: String(url), body: String(options.body ?? "") });
    if (String(options.body ?? "").includes('"method":"getHealth"')) {
      return {
        ok: true,
        json: async () => ({ result: "ok" }),
      };
    }
    return {
      ok: true,
      status: 200,
      json: async () => ({
        paths: { "/v1/gov/proposals/sccp-route-manifest": {} },
      }),
    };
  };
  console.log = () => {};

  try {
    const imported = await import(
      `${new URL("./sccp_solana_taira_xor_deploy.mjs", import.meta.url).href}?env-default-${Date.now()}`
    );
    await imported.main(["doctor"]);
  } finally {
    globalThis.fetch = originalFetch;
    console.log = originalLog;
    if (previousSolanaRpc === undefined) {
      delete process.env.SCCP_SOLANA_TESTNET_RPC_URL;
    } else {
      process.env.SCCP_SOLANA_TESTNET_RPC_URL = previousSolanaRpc;
    }
    if (previousFallbackRpc === undefined) {
      delete process.env.SOLANA_RPC_URL;
    } else {
      process.env.SOLANA_RPC_URL = previousFallbackRpc;
    }
    if (previousTorii === undefined) {
      delete process.env.SCCP_TAIRA_TORII_URL;
    } else {
      process.env.SCCP_TAIRA_TORII_URL = previousTorii;
    }
  }

  assert.deepEqual(
    calls.map((call) => call.url),
    [
      "https://api.testnet.solana.com",
      "https://taira.sora.org/openapi.json",
    ],
  );
  assert.equal(
    calls.some((call) => /secret-token-solana-env/u.test(call.url)),
    false,
  );
});

test("Solana evidence CLI rejects unsafe Solana RPC URLs before spawning", async () => {
  await withTempDir(async (dir) => {
    const outputPath = join(dir, "evidence.json");

    await assert.rejects(
      async () => {
        try {
          await main([
            "evidence",
            "--program-id",
            "Bridge11111111111111111111111111111111111111",
            "--output",
            outputPath,
            "--solana-rpc-url",
            "https://operator:secret-token-solana-rpc-url@api.testnet.solana.com",
          ]);
        } catch (error) {
          assert.doesNotMatch(error.message, /secret-token-solana-rpc-url/u);
          throw error;
        }
      },
      /--solana-rpc-url must not include credentials, params, query strings, or fragments/u,
    );
    await assert.rejects(() => stat(outputPath), /ENOENT/u);
  });
});

test("Solana propose-route-manifest CLI rejects bare manifest before network", async () => {
  await assert.rejects(
    () => main(["propose-route-manifest", "--manifest"]),
    /--manifest must be specified with an explicit value/u,
  );
});

test("Solana propose-route-manifest CLI rejects unsupported modes before reading manifest", async () => {
  await withTempDir(async (dir) => {
    const missingManifestPath = join(dir, "missing-manifest.json");
    const outputPath = join(dir, "proposal.json");

    for (const mode of ["plain", "ZK", "Fast", " Plain"]) {
      await assert.rejects(
        () =>
          main([
            "propose-route-manifest",
            "--manifest",
            missingManifestPath,
            "--mode",
            mode,
            "--output",
            outputPath,
          ]),
        /--mode must be Plain or Zk/u,
      );
      await assert.rejects(() => stat(outputPath), /ENOENT/u);
    }
  });
});

test("Solana propose-route-manifest CLI rejects output path collisions with manifest", async () => {
  await withTempDir(async (dir) => {
    const manifestPath = join(dir, "manifest.json");
    const manifestText = `${JSON.stringify(validTemplate(), null, 2)}\n`;
    await writeFile(manifestPath, manifestText);

    await assert.rejects(
      () =>
        main([
          "propose-route-manifest",
          "--manifest",
          manifestPath,
          "--output",
          manifestPath,
        ]),
      /--output must not be the same path as --manifest/u,
    );
    assert.equal(await readFile(manifestPath, "utf8"), manifestText);

    const linkedInputDir = join(dir, "linked-proposal-inputs");
    await symlink(dir, linkedInputDir);

    await assert.rejects(
      () =>
        main([
          "propose-route-manifest",
          "--manifest",
          manifestPath,
          "--output",
          join(linkedInputDir, "manifest.json"),
        ]),
      /--output must not be the same path as --manifest/u,
    );
    assert.equal(await readFile(manifestPath, "utf8"), manifestText);
  });
});

test("Solana propose-route-manifest CLI rejects padded output before network", async () => {
  await withTempDir(async (dir) => {
    const manifestPath = join(dir, "manifest.json");
    const outputPath = join(dir, "proposal.json");
    await writeJson(manifestPath, validTemplate());
    const originalFetch = globalThis.fetch;
    globalThis.fetch = async () => {
      throw new Error("fetch should not be called");
    };
    try {
      await assert.rejects(
        () =>
          main([
            "propose-route-manifest",
            "--manifest",
            manifestPath,
            "--output",
            `${outputPath} `,
          ]),
        /--output must be a non-empty value without surrounding whitespace/u,
      );
    } finally {
      globalThis.fetch = originalFetch;
    }
    await assert.rejects(() => stat(outputPath), /ENOENT/u);
  });
});

test("Solana propose-route-manifest CLI rejects unsafe Torii URLs before reading manifests", async () => {
  await withTempDir(async (dir) => {
    const missingManifestPath = join(dir, "missing-manifest.json");
    const outputPath = join(dir, "proposal.json");
    const credentialedUrl =
      "https://operator:secret-token-solana-torii-url@taira.sora.org";
    const cases = [
      {
        value: "http://taira.sora.org",
        expected: /--torii-url must use HTTPS unless it is loopback HTTP/u,
      },
      {
        value: credentialedUrl,
        expected:
          /--torii-url must not include credentials, params, query strings, or fragments/u,
      },
      {
        value: "https://taira.sora.org/root;param",
        expected:
          /--torii-url must not include credentials, params, query strings, or fragments/u,
      },
      {
        value: "https://taira.sora.org?private_key=secret-token-solana-torii-url",
        expected:
          /--torii-url must not include credentials, params, query strings, or fragments/u,
      },
      {
        value: "https://taira.sora.org#secret-token-solana-torii-url",
        expected:
          /--torii-url must not include credentials, params, query strings, or fragments/u,
      },
      {
        value: "ftp://taira.sora.org",
        expected: /--torii-url must use HTTPS unless it is loopback HTTP/u,
      },
      {
        value: " https://taira.sora.org",
        expected: /--torii-url must be a valid HTTP\(S\) URL/u,
      },
      {
        value: "https://taira.sora.org\n",
        expected: /--torii-url must be a valid HTTP\(S\) URL/u,
      },
      {
        value: "https://localhost",
        expected: /--torii-url HTTPS host must use public DNS/u,
      },
      {
        value: "https://127.0.0.1",
        expected: /--torii-url HTTPS host must use public DNS/u,
      },
      {
        value: "https://taira",
        expected: /--torii-url HTTPS host must use public DNS/u,
      },
      {
        value: "https://taira.local",
        expected: /--torii-url HTTPS host must use public DNS/u,
      },
      {
        value: "https://bad_host.sora.org",
        expected: /--torii-url HTTPS host must use public DNS/u,
      },
      {
        value: "not a url",
        expected: /--torii-url must be a valid HTTP\(S\) URL/u,
      },
    ];

    for (const testCase of cases) {
      await assert.rejects(
        async () => {
          try {
            await main([
              "propose-route-manifest",
              "--manifest",
              missingManifestPath,
              "--torii-url",
              testCase.value,
              "--output",
              outputPath,
            ]);
          } catch (error) {
            assert.doesNotMatch(
              error.message,
              /secret-token-solana-torii-url/u,
            );
            throw error;
          }
        },
        testCase.expected,
      );
      await assert.rejects(() => stat(outputPath), /ENOENT/u);
    }
  });
});

test("Solana propose-route-manifest CLI accepts loopback HTTP Torii URLs", async () => {
  await withTempDir(async (dir) => {
    const manifestPath = join(dir, "manifest.json");
    const outputPath = join(dir, "proposal.json");
    await writeJson(manifestPath, validTemplate());
    const originalFetch = globalThis.fetch;
    const calls = [];
    globalThis.fetch = async (url, options) => {
      calls.push({ url: String(url), body: JSON.parse(options.body) });
      return {
        ok: true,
        text: async () => JSON.stringify({ proposal_id: "draft-1" }),
      };
    };
    try {
      await main([
        "propose-route-manifest",
        "--manifest",
        manifestPath,
        "--torii-url",
        "http://localhost:8080/",
        "--mode",
        "Zk",
        "--output",
        outputPath,
      ]);
    } finally {
      globalThis.fetch = originalFetch;
    }

    assert.equal(calls.length, 1);
    assert.equal(
      calls[0].url,
      "http://localhost:8080/v1/gov/proposals/sccp-route-manifest",
    );
    assert.equal(calls[0].body.mode, "Zk");
    const payload = JSON.parse(await readFile(outputPath, "utf8"));
    assert.equal(payload.proposal_id, "draft-1");
  });
});

test("Solana propose-route-manifest CLI replaces output symlinks and skips temp symlink collisions", async () => {
  const originalFetch = globalThis.fetch;
  const originalNow = Date.now;
  const originalRandom = Math.random;
  await withTempDir(async (dir) => {
    try {
      const manifestPath = join(dir, "manifest.json");
      await writeJson(manifestPath, validTemplate());
      let proposals = 0;
      globalThis.fetch = async () => {
        proposals += 1;
        return {
          ok: true,
          text: async () => JSON.stringify({ proposal_id: `draft-${proposals}` }),
        };
      };

      const outputPath = join(dir, "proposal.json");
      const targetPath = join(dir, "proposal-target.json");
      const outputSentinel = "sentinel:solana-proposal-output-target\n";
      await writeFile(targetPath, outputSentinel);
      await symlink(targetPath, outputPath);

      await main([
        "propose-route-manifest",
        "--manifest",
        manifestPath,
        "--torii-url",
        "http://localhost:8080/",
        "--output",
        outputPath,
      ]);

      assert.equal(await readFile(targetPath, "utf8"), outputSentinel);
      assert.equal((await lstat(outputPath)).isSymbolicLink(), false);
      assert.equal(
        JSON.parse(await readFile(outputPath, "utf8")).proposal_id,
        "draft-1",
      );

      const tempOutput = join(dir, "proposal-temp.json");
      const trapTarget = join(dir, "proposal-temp-target.json");
      const tempSentinel = "sentinel:solana-proposal-temp-target\n";
      await writeFile(trapTarget, tempSentinel);
      const tempOne = `${tempOutput}.tmp-${process.pid}.424242.8`;
      const tempTwo = `${tempOutput}.tmp-${process.pid}.424242.c`;
      await symlink(trapTarget, tempOne);
      await symlink(trapTarget, tempTwo);
      const forcedRandoms = [0.5, 0.75, 0.875];
      Date.now = () => 424242;
      Math.random = () => forcedRandoms.shift() ?? 0.875;

      await main([
        "propose-route-manifest",
        "--manifest",
        manifestPath,
        "--torii-url",
        "http://localhost:8080/",
        "--output",
        tempOutput,
      ]);

      assert.equal(await readFile(trapTarget, "utf8"), tempSentinel);
      await assert.rejects(() => stat(tempOne), /ENOENT/u);
      await assert.rejects(() => stat(tempTwo), /ENOENT/u);
      assert.equal(
        JSON.parse(await readFile(tempOutput, "utf8")).proposal_id,
        "draft-2",
      );
    } finally {
      globalThis.fetch = originalFetch;
      Date.now = originalNow;
      Math.random = originalRandom;
    }
  });
});

test("Solana route-manifest CLI replaces output symlinks instead of following them", async () => {
  await withTempDir(async (dir) => {
    const templatePath = join(dir, "template.json");
    const evidencePath = join(dir, "evidence.json");
    const outputPath = join(dir, "manifest.json");
    const targetPath = join(dir, "sentinel-target.json");
    const sentinel = "sentinel:solana-route-manifest-target\n";
    await writeJson(templatePath, validTemplate());
    await writeJson(evidencePath, validEvidence());
    await writeFile(targetPath, sentinel);
    await symlink(targetPath, outputPath);

    await main([
      "route-manifest",
      "--template",
      templatePath,
      "--evidence",
      evidencePath,
      "--output",
      outputPath,
    ]);

    assert.equal(await readFile(targetPath, "utf8"), sentinel);
    assert.equal((await lstat(outputPath)).isSymbolicLink(), false);
    const manifest = JSON.parse(await readFile(outputPath, "utf8"));
    assert.equal(manifest.production_ready, true);
  });
});

test("Solana route-manifest CLI skips hostile temp symlink collisions", async () => {
  const originalNow = Date.now;
  const originalRandom = Math.random;
  await withTempDir(async (dir) => {
    try {
      const templatePath = join(dir, "template.json");
      const evidencePath = join(dir, "evidence.json");
      const outputPath = join(dir, "manifest.json");
      const trapTarget = join(dir, "temp-target.json");
      const sentinel = "sentinel:solana-route-temp-target\n";
      await writeJson(templatePath, validTemplate());
      await writeJson(evidencePath, validEvidence());
      await writeFile(trapTarget, sentinel);
      const tempOne = `${outputPath}.tmp-${process.pid}.424242.8`;
      const tempTwo = `${outputPath}.tmp-${process.pid}.424242.c`;
      await symlink(trapTarget, tempOne);
      await symlink(trapTarget, tempTwo);
      const forcedRandoms = [0.5, 0.75, 0.875];
      Date.now = () => 424242;
      Math.random = () => forcedRandoms.shift() ?? 0.875;

      await main([
        "route-manifest",
        "--template",
        templatePath,
        "--evidence",
        evidencePath,
        "--output",
        outputPath,
      ]);

      assert.equal(await readFile(trapTarget, "utf8"), sentinel);
      await assert.rejects(() => stat(tempOne), /ENOENT/u);
      await assert.rejects(() => stat(tempTwo), /ENOENT/u);
      const manifest = JSON.parse(await readFile(outputPath, "utf8"));
      assert.equal(manifest.production_ready, true);
    } finally {
      Date.now = originalNow;
      Math.random = originalRandom;
    }
  });
});
