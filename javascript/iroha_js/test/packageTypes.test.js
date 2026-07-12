import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import ts from "typescript";

import { validatePackPaths } from "../scripts/package-install-smoke.mjs";

const PACKAGE_ROOT = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const TSC = fileURLToPath(
  new URL("../node_modules/typescript/bin/tsc", import.meta.url),
);
const PACKAGE_NAME = "@iroha/iroha-js";
const PORTABLE_RECIPES = [
  "recipes/iso_bridge_builder.mjs",
  "recipes/nexus_app_transfer.mjs",
];

function readPackageJson() {
  return JSON.parse(
    fs.readFileSync(path.join(PACKAGE_ROOT, "package.json"), "utf8"),
  );
}

function assertSafePackageTarget(target, context) {
  assert.equal(typeof target, "string", `${context} must be a string`);
  assert.match(target, /^\.\/[A-Za-z0-9._/-]+$/u, `${context} is unsafe`);
  assert.equal(target.includes(".."), false, `${context} traverses the package`);
  const absolute = path.resolve(PACKAGE_ROOT, target);
  assert.equal(
    absolute.startsWith(`${PACKAGE_ROOT}${path.sep}`),
    true,
    `${context} escapes the package`,
  );
  assert.equal(fs.statSync(absolute).isFile(), true, `${context} is not a file`);
}

function createPackedLayout({ includeNodeTypes }) {
  const tempRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), "iroha-js-packed-types-"),
  );
  const nodeModules = path.join(tempRoot, "node_modules");
  const packagePath = path.join(nodeModules, "@iroha", "iroha-js");
  fs.mkdirSync(packagePath, { recursive: true });
  fs.copyFileSync(
    path.join(PACKAGE_ROOT, "package.json"),
    path.join(packagePath, "package.json"),
  );
  for (const entry of fs.readdirSync(PACKAGE_ROOT, { withFileTypes: true })) {
    if (entry.isFile() && entry.name.endsWith(".d.ts")) {
      fs.copyFileSync(
        path.join(PACKAGE_ROOT, entry.name),
        path.join(packagePath, entry.name),
      );
    }
  }
  fs.cpSync(path.join(PACKAGE_ROOT, "dist"), path.join(packagePath, "dist"), {
    recursive: true,
  });
  fs.symlinkSync(
    path.join(PACKAGE_ROOT, "node_modules", "buffer"),
    path.join(nodeModules, "buffer"),
    "dir",
  );
  if (includeNodeTypes) {
    const atTypes = path.join(nodeModules, "@types");
    fs.mkdirSync(atTypes, { recursive: true });
    fs.symlinkSync(
      path.join(PACKAGE_ROOT, "node_modules", "@types", "node"),
      path.join(atTypes, "node"),
      "dir",
    );
    fs.symlinkSync(
      path.join(PACKAGE_ROOT, "node_modules", "undici-types"),
      path.join(nodeModules, "undici-types"),
      "dir",
    );
  }
  fs.writeFileSync(
    path.join(tempRoot, "package.json"),
    `${JSON.stringify({ private: true, type: "module" })}\n`,
    "utf8",
  );
  return { tempRoot, packagePath };
}

function compileFixture(tempRoot, tsconfig) {
  fs.writeFileSync(
    path.join(tempRoot, "tsconfig.json"),
    `${JSON.stringify(tsconfig, null, 2)}\n`,
    "utf8",
  );
  return spawnSync(process.execPath, [TSC, "-p", "tsconfig.json", "--pretty", "false"], {
    cwd: tempRoot,
    encoding: "utf8",
  });
}

test("every public export has a safe runtime target and an explicit declaration target", () => {
  const packageJson = readPackageJson();
  const legacyTypes = packageJson.typesVersions["*"];
  const expectedLegacyKeys = [];

  for (const [subpath, descriptor] of Object.entries(packageJson.exports)) {
    assert.equal(
      descriptor !== null && typeof descriptor === "object" && !Array.isArray(descriptor),
      true,
      `${subpath} must use conditional exports with an explicit types target`,
    );
    assertSafePackageTarget(descriptor.import, `${subpath}.import`);
    if (descriptor.browser !== undefined) {
      assertSafePackageTarget(descriptor.browser, `${subpath}.browser`);
    }
    assertSafePackageTarget(descriptor.types, `${subpath}.types`);
    assert.match(descriptor.types, /\.d\.ts$/u, `${subpath}.types must be a declaration`);

    if (subpath !== ".") {
      const legacyKey = subpath.slice(2);
      expectedLegacyKeys.push(legacyKey);
      assert.deepEqual(
        legacyTypes[legacyKey],
        [descriptor.types],
        `${subpath} typesVersions target diverges from exports`,
      );
    }
  }

  assert.deepEqual(
    Object.keys(legacyTypes).sort(),
    expectedLegacyKeys.sort(),
    "typesVersions must cover every public subpath exactly once",
  );
  assert.deepEqual(packageJson.exports["./norito"], {
    import: "./dist/norito.js",
    types: "./index.d.ts",
  });
});

test("the registry package allowlists only clean-install portable recipes", () => {
  const packageJson = readPackageJson();
  const fileRecipes = packageJson.files
    .filter((entry) => entry === "recipes" || entry.startsWith("recipes/"))
    .sort();
  assert.deepEqual(fileRecipes, [...PORTABLE_RECIPES].sort());
  assert.equal(packageJson.files.includes("recipes"), false);

  const sourceRecipes = fs
    .readdirSync(path.join(PACKAGE_ROOT, "recipes"), { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(".mjs"))
    .map((entry) => `recipes/${entry.name}`)
    .sort();
  assert.deepEqual(
    sourceRecipes.filter((entry) => PORTABLE_RECIPES.includes(entry)).sort(),
    [...PORTABLE_RECIPES].sort(),
  );
  assert.ok(
    sourceRecipes.some((entry) => !PORTABLE_RECIPES.includes(entry)),
    "fixture must retain a forbidden source-checkout-only recipe",
  );
});

test("published recipe documentation exactly matches the portable allowlist", () => {
  const rootReadme = fs.readFileSync(path.join(PACKAGE_ROOT, "README.md"), "utf8");
  const recipeReadme = fs.readFileSync(
    path.join(PACKAGE_ROOT, "recipes", "README.md"),
    "utf8",
  );
  const rootContract = /The registry artifact includes only(?<body>[\s\S]*?)The wider recipe catalog/u.exec(
    rootReadme,
  );
  const recipeContract = /The portable registry\s+tarball includes only(?<body>[\s\S]*?);\s+examples that require/u.exec(
    recipeReadme,
  );
  assert.ok(rootContract?.groups?.body, "root README lacks the registry recipe contract");
  assert.ok(
    recipeContract?.groups?.body,
    "recipes README lacks the registry recipe contract",
  );

  const rootRecipes = [
    ...rootContract.groups.body.matchAll(/recipes\/[a-z0-9_]+\.mjs/gu),
  ].map((match) => match[0]);
  const recipeRecipes = [
    ...recipeContract.groups.body.matchAll(/[a-z0-9_]+\.mjs/gu),
  ].map((match) => `recipes/${match[0]}`);
  assert.deepEqual(rootRecipes.sort(), [...PORTABLE_RECIPES].sort());
  assert.deepEqual(recipeRecipes.sort(), [...PORTABLE_RECIPES].sort());
});

test("package smoke rejects every non-portable or missing required recipe", () => {
  const requiredPaths = [
    "package.json",
    "ivm-artifact.d.ts",
    "src/index.js",
    "dist/index.js",
    "dist/ivmArtifact.js",
    "dist/nexusApp.js",
    "nexus-app.d.ts",
    ...PORTABLE_RECIPES,
    "scripts/build-dist.mjs",
  ];
  const metadata = {
    files: requiredPaths.map((entry) => ({ path: entry })),
  };
  assert.doesNotThrow(() => validatePackPaths(metadata));

  for (const recipe of PORTABLE_RECIPES) {
    assert.throws(
      () =>
        validatePackPaths({
          files: metadata.files.filter((entry) => entry.path !== recipe),
        }),
      new RegExp(`missing required tar entry: ${recipe.replace(".", "\\.")}`, "u"),
    );
  }
  for (const forbidden of [
    "recipes/batching.mjs",
    "recipes/contracts.mjs",
    "recipes/governance.mjs",
    "recipes/walletlessFollowGame.mjs",
  ]) {
    assert.throws(
      () =>
        validatePackPaths({
          files: [...metadata.files, { path: forbidden }],
        }),
      new RegExp(`forbidden non-portable recipe: ${forbidden.replace(".", "\\.")}`, "u"),
    );
  }
});

test("runtime namespace declarations expose exactly their module exports", async () => {
  const declarationPath = path.join(PACKAGE_ROOT, "index.d.ts");
  const program = ts.createProgram([declarationPath], {
    strict: true,
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.NodeNext,
    moduleResolution: ts.ModuleResolutionKind.NodeNext,
    skipLibCheck: false,
  });
  const declarations = program.getSourceFile(declarationPath);
  assert.ok(declarations);
  const checker = program.getTypeChecker();
  const moduleSymbol = checker.getSymbolAtLocation(declarations);
  assert.ok(moduleSymbol);
  const declarationExports = new Map(
    checker.getExportsOfModule(moduleSymbol).map((symbol) => [symbol.name, symbol]),
  );

  for (const [namespaceName, moduleName] of [
    ["Torii", "toriiClient"],
    ["Norito", "norito"],
    ["Crypto", "crypto"],
    ["OfflineQrStream", "offlineQrStream"],
  ]) {
    const namespaceSymbol = declarationExports.get(namespaceName);
    assert.ok(namespaceSymbol, `missing ${namespaceName} declaration`);
    const namespaceType = checker.getTypeOfSymbolAtLocation(
      namespaceSymbol,
      namespaceSymbol.valueDeclaration ?? declarations,
    );
    const declaredNames = checker
      .getPropertiesOfType(namespaceType)
      .map((symbol) => symbol.name)
      .sort();
    const runtimeModule = await import(`../src/${moduleName}.js`);
    assert.deepEqual(
      declaredNames,
      Object.keys(runtimeModule).sort(),
      `${namespaceName} declaration diverges from ${moduleName}.js`,
    );
  }
});

test("strict NodeNext resolves the root and every public subpath from a packed layout", () => {
  const packageJson = readPackageJson();
  const { tempRoot } = createPackedLayout({ includeNodeTypes: true });
  try {
    const imports = Object.keys(packageJson.exports).map((subpath, index) => {
      const specifier = subpath === "." ? PACKAGE_NAME : `${PACKAGE_NAME}/${subpath.slice(2)}`;
      return `import * as export${index} from ${JSON.stringify(specifier)};`;
    });
    const bindings = Object.keys(packageJson.exports).map((_, index) => `export${index}`);
    fs.writeFileSync(
      path.join(tempRoot, "consumer.mts"),
      [
        ...imports,
        `import { Crypto, Norito, OfflineQrStream, Torii, ToriiClient, type CryptoAlgorithm } from ${JSON.stringify(PACKAGE_NAME)};`,
        'const algorithm: CryptoAlgorithm = "ed25519";',
        "const toriiConstructor: typeof ToriiClient = Torii.ToriiClient;",
        "const encodeInstruction: typeof export10.noritoEncodeInstruction = Norito.noritoEncodeInstruction;",
        "const generateKeyPair: typeof export15.generateKeyPair = Crypto.generateKeyPair;",
        "const streamEncoder: typeof OfflineQrStream.OfflineQrStreamEncoder = OfflineQrStream.OfflineQrStreamEncoder;",
        "// @ts-expect-error Torii does not expose crypto helpers.",
        "void Torii.generateKeyPair;",
        "// @ts-expect-error Crypto does not expose Torii clients.",
        "void Crypto.ToriiClient;",
        "// @ts-expect-error Norito does not expose crypto helpers.",
        "void Norito.generateKeyPair;",
        "// @ts-expect-error offline QR helpers do not expose Torii clients.",
        "void OfflineQrStream.ToriiClient;",
        `void [${bindings.join(", ")}];`,
        "void algorithm; void toriiConstructor; void encodeInstruction; void generateKeyPair; void streamEncoder;",
      ].join("\n"),
      "utf8",
    );
    const compiled = compileFixture(tempRoot, {
      compilerOptions: {
        strict: true,
        exactOptionalPropertyTypes: true,
        noUncheckedIndexedAccess: true,
        skipLibCheck: false,
        target: "ES2022",
        module: "NodeNext",
        moduleResolution: "NodeNext",
        types: ["node"],
        noEmit: true,
      },
      files: ["consumer.mts"],
    });
    assert.equal(
      compiled.status,
      0,
      `strict packed NodeNext compile failed:\n${compiled.stdout}\n${compiled.stderr}`,
    );
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
});

test("dedicated browser declarations compile without ambient Node types", () => {
  const { tempRoot } = createPackedLayout({ includeNodeTypes: false });
  try {
    assert.equal(fs.existsSync(path.join(tempRoot, "node_modules", "@types", "node")), false);
    fs.writeFileSync(
      path.join(tempRoot, "browser-consumer.ts"),
      [
        'import { createConnectAppSession, type BrowserConnectApproval } from "@iroha/iroha-js/connect-browser";',
        'import { NexusAppClient, type NexusFinalizeOptions } from "@iroha/iroha-js/nexus-app";',
        'import { browserTransactionCodec } from "@iroha/iroha-js/transaction-codec";',
        'import { computeIvmArtifactHashes } from "@iroha/iroha-js/ivm-artifact";',
        'import { canonicalQueryString } from "@iroha/iroha-js/canonical-request";',
        'import { compileKotodamaProgram } from "@iroha/iroha-js/kotodama-compiler";',
        "declare const approval: BrowserConnectApproval;",
        'const options: NexusFinalizeOptions = { wait: true, signal: new AbortController().signal };',
        "const bytes = new Uint8Array(17);",
        "void createConnectAppSession; void approval; void options;",
        "void new NexusAppClient({ transactionCodec: browserTransactionCodec });",
        "void computeIvmArtifactHashes(bytes);",
        'void canonicalQueryString(new URLSearchParams({ browser: "true" }));',
        'void compileKotodamaProgram("CREATE DOMAIN browser");',
      ].join("\n"),
      "utf8",
    );
    const compiled = compileFixture(tempRoot, {
      compilerOptions: {
        strict: true,
        exactOptionalPropertyTypes: true,
        noUncheckedIndexedAccess: true,
        skipLibCheck: false,
        target: "ES2022",
        module: "ESNext",
        moduleResolution: "Bundler",
        lib: ["ES2022", "DOM", "DOM.Iterable"],
        types: [],
        noEmit: true,
      },
      files: ["browser-consumer.ts"],
    });
    assert.equal(
      compiled.status,
      0,
      `strict packed browser compile failed:\n${compiled.stdout}\n${compiled.stderr}`,
    );
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
});
