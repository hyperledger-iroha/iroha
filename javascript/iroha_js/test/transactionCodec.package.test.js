import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import * as browserAggregate from "../dist/browser.js";
import * as distRoot from "../dist/index.js";
import * as distSubpath from "../dist/transactionCodec.js";
import * as sourceSubpath from "../src/transactionCodec.js";
import * as packageSubpath from "@iroha/iroha-js/transaction-codec";

const EXPECTED_EXPORTS = [
  "BrowserTransactionCodecError",
  "browserSignedTransactionHashHex",
  "browserTransactionCodec",
  "browserTransactionPayloadHashHex",
  "buildBrowserExecutableBatchPayload",
  "buildBrowserTransferPayload",
  "finalizeBrowserExecutableBatchTransaction",
  "finalizeBrowserSignedTransaction",
  "validateBrowserExecutableBatchSignable",
  "validateBrowserTransferSignable",
];

test("transaction codec is identical across root, browser, dist, and package subpath exports", () => {
  for (const name of EXPECTED_EXPORTS) {
    assert.equal(typeof distSubpath[name], name.endsWith("Codec") ? "object" : "function");
    assert.equal(distRoot[name], distSubpath[name], `root export mismatch for ${name}`);
    assert.equal(
      browserAggregate[name],
      distSubpath[name],
      `browser aggregate export mismatch for ${name}`,
    );
    assert.equal(
      packageSubpath[name],
      distSubpath[name],
      `package subpath export mismatch for ${name}`,
    );
  }
});

test("source and packaged codecs reject every retired ordinary-transaction chain alias", () => {
  for (const codec of [sourceSubpath, distSubpath, distRoot, browserAggregate, packageSubpath]) {
    for (const field of ["chain", "chainId", "chain_id"]) {
      assert.throws(
        () => codec.buildBrowserTransferPayload({ [field]: "bad\ud800" }),
        (error) => error?.code === "invalid_input",
        field,
      );
    }
  }
});

test("package metadata and declarations expose the browser transaction codec", () => {
  const packageJson = JSON.parse(
    fs.readFileSync(new URL("../package.json", import.meta.url), "utf8"),
  );
  assert.deepEqual(packageJson.exports["./transaction-codec"], {
    browser: "./dist/transactionCodec.js",
    import: "./dist/transactionCodec.js",
    types: "./transaction-codec.d.ts",
  });
  assert.deepEqual(packageJson.typesVersions["*"]["transaction-codec"], [
    "./transaction-codec.d.ts",
  ]);
  assert.ok(packageJson.files.includes("transaction-codec.d.ts"));

  const declarations = fs.readFileSync(
    new URL("../transaction-codec.d.ts", import.meta.url),
    "utf8",
  );
  for (const name of EXPECTED_EXPORTS) {
    assert.match(declarations, new RegExp(`\\b${name}\\b`, "u"));
  }
  const rootDeclarations = fs.readFileSync(
    new URL("../index.d.ts", import.meta.url),
    "utf8",
  );
  assert.match(rootDeclarations, /export \* from "\.\/transaction-codec\.js";/u);
});

test("strict TypeScript accepts the browser codec in NexusAppConfig", () => {
  const tsc = fileURLToPath(
    new URL("../node_modules/typescript/bin/tsc", import.meta.url),
  );
  const fixture = fileURLToPath(
    new URL("../fixtures/typescript/transactionCodec.types.ts", import.meta.url),
  );
  const result = spawnSync(
    process.execPath,
    [
      tsc,
      "--noEmit",
      "--strict",
      "--exactOptionalPropertyTypes",
      "--noUncheckedIndexedAccess",
      "--target",
      "ES2022",
      "--module",
      "NodeNext",
      "--moduleResolution",
      "NodeNext",
      fixture,
    ],
    { cwd: fileURLToPath(new URL("..", import.meta.url)), encoding: "utf8" },
  );
  assert.equal(
    result.status,
    0,
    `tsc failed:\n${result.stdout}\n${result.stderr}`,
  );
});

test("packed browser declarations compile without ambient Node types", () => {
  const packageRoot = fileURLToPath(new URL("..", import.meta.url));
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "iroha-js-browser-types-"));
  try {
    const packed = spawnSync(
      "npm",
      [
        "pack",
        "--ignore-scripts",
        "--json",
        "--pack-destination",
        tempRoot,
      ],
      { cwd: packageRoot, encoding: "utf8" },
    );
    assert.equal(
      packed.status,
      0,
      `npm pack failed:\n${packed.stdout}\n${packed.stderr}`,
    );
    const packResult = JSON.parse(packed.stdout);
    const tarball = path.join(tempRoot, packResult[0].filename);
    const extracted = spawnSync("tar", ["-xzf", tarball, "-C", tempRoot], {
      encoding: "utf8",
    });
    assert.equal(
      extracted.status,
      0,
      `tar extraction failed:\n${extracted.stdout}\n${extracted.stderr}`,
    );

    const nodeModules = path.join(tempRoot, "node_modules");
    const scope = path.join(nodeModules, "@iroha");
    fs.mkdirSync(scope, { recursive: true });
    fs.renameSync(path.join(tempRoot, "package"), path.join(scope, "iroha-js"));
    fs.symlinkSync(
      path.join(packageRoot, "node_modules", "buffer"),
      path.join(nodeModules, "buffer"),
      "dir",
    );
    fs.writeFileSync(
      path.join(tempRoot, "consumer.ts"),
      [
        'import { NetworkId } from "@iroha/iroha-js";',
        'import { NexusAppClient, type NexusFinalizeOptions, type NexusTransactionCodec } from "@iroha/iroha-js/nexus-app";',
        'import type { BrowserConnectApproval } from "@iroha/iroha-js/connect-browser";',
        'import { browserTransactionCodec, buildBrowserTransferPayload } from "@iroha/iroha-js/transaction-codec";',
        "const codec: NexusTransactionCodec = browserTransactionCodec;",
        "new NexusAppClient({ transactionCodec: codec });",
        'const networkId = NetworkId.parse("hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");',
        "const bytes: Uint8Array = buildBrowserTransferPayload({",
        '  networkId, authority: "authority", sourceAssetHoldingId: "asset#authority",',
        '  quantity: "1", destinationAccountId: "destination",',
        '  feePayment: { payer: "authority", chargeLimits: [] },',
        "});",
        "void bytes;",
        "const waitOptions: NexusFinalizeOptions = { wait: true, signal: new AbortController().signal };",
        "const noWaitOptions: NexusFinalizeOptions = { wait: false };",
        "// @ts-expect-error no-wait submissions cannot accept a polling signal.",
        "const invalidNoWaitOptions: NexusFinalizeOptions = { wait: false, signal: new AbortController().signal };",
        "// @ts-expect-error execution success is fixed to exact Applied and cannot be overridden.",
        'const invalidStringStatuses: NexusFinalizeOptions = { wait: true, successStatuses: new Set(["Committed"]) };',
        "// @ts-expect-error Rejected and Expired are fixed failures.",
        'const invalidFailureStatuses: NexusFinalizeOptions = { wait: true, failureStatuses: new Set(["Applied"]) };',
        "// @ts-expect-error callers cannot define an alternate terminal set.",
        'const invalidTerminalStatuses: NexusFinalizeOptions = { wait: true, terminalStatuses: new Set(["Committed"]) };',
        "declare const approval: BrowserConnectApproval;",
        "// @ts-expect-error approval wrappers expose readonly fields.",
        'approval.accountId = "replacement";',
        "void waitOptions;",
        "void noWaitOptions;",
        "void invalidNoWaitOptions;",
        "void invalidStringStatuses;",
        "void invalidFailureStatuses;",
        "void invalidTerminalStatuses;",
      ].join("\n"),
      "utf8",
    );
    fs.writeFileSync(
      path.join(tempRoot, "tsconfig.json"),
      JSON.stringify(
        {
          compilerOptions: {
            strict: true,
            exactOptionalPropertyTypes: true,
            noUncheckedIndexedAccess: true,
            target: "ES2022",
            module: "NodeNext",
            moduleResolution: "NodeNext",
            lib: ["ES2022", "DOM"],
            types: [],
            noEmit: true,
          },
          files: ["consumer.ts"],
        },
        null,
        2,
      ),
      "utf8",
    );
    const tsc = fileURLToPath(
      new URL("../node_modules/typescript/bin/tsc", import.meta.url),
    );
    const compiled = spawnSync(process.execPath, [tsc, "-p", "tsconfig.json"], {
      cwd: tempRoot,
      encoding: "utf8",
    });
    assert.equal(
      compiled.status,
      0,
      `packed browser declaration compile failed:\n${compiled.stdout}\n${compiled.stderr}`,
    );
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
});
