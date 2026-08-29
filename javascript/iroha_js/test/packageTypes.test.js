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
  fs.cpSync(path.join(PACKAGE_ROOT, "src"), path.join(packagePath, "src"), {
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

test("retired generic confidential declarations are absent from the published surface", () => {
  const declarations = fs.readFileSync(path.join(PACKAGE_ROOT, "index.d.ts"), "utf8");
  const retiredDeclarations = [
    ["ConfidentialEncryptedPayload", "Input"],
    ...[["Shi", "eld"], ["Zk", "Transfer"], ["Un", "shield"]].flatMap(
      (variantParts) => {
        const variant = variantParts.join("");
        return [
          [variant, "InstructionInput"],
          [variant, "TransactionInput"],
          ["build", variant, "Instruction"],
          ["build", variant, "Transaction"],
        ];
      },
    ),
  ].map((parts) => parts.join(""));
  for (const retired of retiredDeclarations) {
    assert.doesNotMatch(declarations, new RegExp(`\\b${retired}\\b`, "u"), retired);
  }
});

test("Sumeragi V2 declarations use canonical Rust names without draft aliases", () => {
  const declarations = fs.readFileSync(path.join(PACKAGE_ROOT, "index.d.ts"), "utf8");
  for (const canonical of [
    "ToriiSumeragiV2HeightContextId",
    "ToriiSumeragiV2ConsensusRound",
    "ToriiSumeragiV2QuorumCertificateRef",
    "ToriiSumeragiV2TimeoutCertificateRef",
  ]) {
    assert.match(
      declarations,
      new RegExp(`export (?:type|interface) ${canonical}\\b`, "u"),
      `missing canonical ${canonical} declaration`,
    );
  }
  for (const retired of [
    "ToriiSumeragiV2ContextId",
    "ToriiSumeragiV2Round",
    "ToriiSumeragiV2QcReference",
    "ToriiSumeragiV2TimeoutReference",
  ]) {
    assert.doesNotMatch(
      declarations,
      new RegExp(`export (?:type|interface) ${retired}\\b`, "u"),
      `retired draft alias ${retired} must be absent`,
    );
  }
});

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

test("package smoke rejects every non-portable or missing required artifact", () => {
  const requiredLazyPaths = [
    "dist/smartContractDeploymentSubmit.js",
    "dist/sumeragiTyped.js",
  ];
  const requiredPaths = [
    "package.json",
    "atomic-private-settlement.d.ts",
    "index.d.ts",
    "browser.d.ts",
    "ivm-artifact.d.ts",
    "kotodama-compiler.d.ts",
    "privacy-capabilities.d.ts",
    "repo-agreement.d.ts",
    "sumeragi-typed.d.ts",
    "src/index.js",
    "dist/index.js",
    "dist/atomicPrivateSettlement.js",
    "dist/ivmArtifact.js",
    "dist/kotodamaCompiler/browser.js",
    "dist/kotodamaCompiler/index.js",
    "dist/nexusApp.js",
    "dist/privacyCapabilities.js",
    "dist/sorafsOrderbookSubmission.js",
    "dist/sorafsOrderbookSubmission.d.ts",
    "dist/tairaTestnetProfile.js",
    ...requiredLazyPaths,
    "nexus-app.d.ts",
    ...PORTABLE_RECIPES,
    "scripts/build-dist.mjs",
  ];
  const metadata = {
    files: requiredPaths.map((entry) => ({ path: entry })),
  };
  assert.doesNotThrow(() => validatePackPaths(metadata));

  for (const requiredPath of [
    "index.d.ts",
    "atomic-private-settlement.d.ts",
    "dist/atomicPrivateSettlement.js",
    "dist/tairaTestnetProfile.js",
    ...PORTABLE_RECIPES,
    ...requiredLazyPaths,
  ]) {
    assert.throws(
      () =>
        validatePackPaths({
          files: metadata.files.filter((entry) => entry.path !== requiredPath),
        }),
      new RegExp(
        `missing required tar entry: ${requiredPath.replaceAll(".", "\\.")}`,
        "u",
      ),
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

test("SoraFS gateway denial declarations expose only governed catalog evidence", () => {
  const declarationPath = path.join(PACKAGE_ROOT, "index.d.ts");
  const declarationText = fs.readFileSync(declarationPath, "utf8");
  const source = ts.createSourceFile(
    declarationPath,
    declarationText,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TS,
  );
  const findInterface = (name) => {
    const declaration = source.statements.find(
      (statement) =>
        ts.isInterfaceDeclaration(statement) && statement.name.text === name,
    );
    assert.ok(declaration, `missing ${name} declaration`);
    return declaration;
  };
  const propertyName = (member) => member.name?.getText(source);

  const options = findInterface("SorafsGatewayFetchOptions");
  const optionNames = options.members.map(propertyName);
  assert.ok(optionNames.includes("cacheVersion"));
  assert.equal(optionNames.includes("moderationTokenKey"), false);
  const exactOptionTypes = new Map([
    ["rolloutPhase", '"canary" | "ramp" | "default"'],
    [
      "transportPolicy",
      '"soranet-first" | "soranet-strict" | "direct-only"',
    ],
    [
      "anonymityPolicy",
      '"anon-guard-pq" | "anon-majority-pq" | "anon-strict-pq"',
    ],
    ["writeMode", '"read-only" | "upload-pq-only"'],
  ]);
  for (const [name, expectedType] of exactOptionTypes) {
    const property = options.members.find(
      (member) => propertyName(member) === name,
    );
    assert.ok(property && ts.isPropertySignature(property), `missing ${name}`);
    assert.equal(
      property.type?.getText(source),
      expectedType,
      `${name} must expose only exact V1 labels`,
    );
  }

  const failure = findInterface("SorafsGatewayFetchAttemptFailure");
  const policyBlock = failure.members.find(
    (member) => propertyName(member) === "policyBlock",
  );
  assert.ok(policyBlock && ts.isPropertySignature(policyBlock));
  assert.ok(
    policyBlock.type && ts.isTypeLiteralNode(policyBlock.type),
    "policyBlock must be a closed type literal",
  );
  assert.deepEqual(policyBlock.type.members.map(propertyName), [
    "observedStatus",
    "code",
    "source",
    "catalogDigestHex",
  ]);
  assert.equal(
    policyBlock.type.members[0].type?.getText(source),
    "451",
    "the observed denial status must be exact",
  );
  assert.equal(
    policyBlock.type.members[1].type?.getText(source),
    '"gateway_compliance_denied"',
    "the denial code must be exact",
  );
  for (const removed of [
    "canonicalStatus",
    "cacheVersion",
    "denylistVersion",
    "proofTokenPresent",
    "message",
  ]) {
    assert.equal(
      policyBlock.type.members.some((member) => propertyName(member) === removed),
      false,
      `removed denial field ${removed} must stay absent`,
    );
  }
});

test("strict NodeNext resolves the root and every public subpath from a packed layout", () => {
  const packageJson = readPackageJson();
  const rootDeclarations = fs.readFileSync(
    path.join(PACKAGE_ROOT, "index.d.ts"),
    "utf8",
  );
  const blockProofDeclarations = fs.readFileSync(
    path.join(PACKAGE_ROOT, "src", "blockProofTypes.d.ts"),
    "utf8",
  );
  for (const name of [
    "ToriiBlockMerkleCommitment",
    "ToriiBlockMerkleProof",
    "ToriiBlockProofs",
    "ToriiBlockProofTrustedAnchor",
    "ToriiBlockProofVerification",
  ]) {
    assert.doesNotMatch(
      rootDeclarations,
      new RegExp(`export interface ${name}\\b`, "u"),
      `${name} must have one canonical declaration source`,
    );
    assert.match(
      blockProofDeclarations,
      new RegExp(`export interface ${name}\\b`, "u"),
      `missing canonical ${name} declaration`,
    );
  }
  assert.match(
    blockProofDeclarations,
    /readonly networkId: NetworkId;/u,
    "authenticated BlockProofs must bind the nominal exact NetworkId",
  );
  assert.doesNotMatch(
    blockProofDeclarations,
    /readonly chainId:/u,
    "retired ChainId spelling must stay absent",
  );
  const indexOfSubpath = (subpath) => Object.keys(packageJson.exports).indexOf(subpath);
  const noritoIndex = indexOfSubpath("./norito");
  const cryptoIndex = indexOfSubpath("./crypto");
  const browserIndex = indexOfSubpath("./browser");
  assert.notEqual(noritoIndex, -1);
  assert.notEqual(cryptoIndex, -1);
  assert.notEqual(browserIndex, -1);
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
        `import * as RootSdk from ${JSON.stringify(PACKAGE_NAME)};`,
        `import { Crypto, Norito, NumericV1, SorafsOrderbookSubmissionAmbiguousError, Torii, ToriiBrowserClient, ToriiClient, CANCEL_ASSET_LOCK_MAX_LOCK_ID_UTF8_BYTES_V1, buildCancelAssetLockInstruction, buildSetAssetTransferAvailabilityInstruction, decodeCancelAssetLockV1, encodeCancelAssetLockV1, validateAppealFinanceCancelAssetLock, type AssetTransferAvailability, type CancelAssetLockInstruction, type CancelAssetLockV1, type CancelAssetLockV1Archive, type CanonicalRequestAuth, type ContractEntrypointValueKindName, type CryptoAlgorithm, type IdentifierClaimLookupResponse, type IdentifierPolicyListResponse, type IdentifierResolutionReceipt, type PrivacyEngineIdV1, type PrivacyProofSystemIdV1, type RamLfeExecuteResponse, type RamLfeOutputOpening, type SetAssetTransferAvailabilityInstruction, type SorafsOrderbookSubmissionReceipt, type SorafsValidationOutcome, type ToriiRepoAgreement, type ToriiVerifierBackendLabelV1 } from ${JSON.stringify(PACKAGE_NAME)};`,
        `import { getPrivacyCapabilitiesV1, parsePrivacyCapabilitySnapshotV1, type PrivacyCapabilitySnapshotV1 } from ${JSON.stringify(`${PACKAGE_NAME}/privacy-capabilities`)};`,
        'const algorithm: CryptoAlgorithm = "ed25519";',
        "const cancelAssetLockMaxLockIdUtf8BytesV1: 4096 = CANCEL_ASSET_LOCK_MAX_LOCK_ID_UTF8_BYTES_V1;",
        'const cancelAssetLock: CancelAssetLockInstruction = buildCancelAssetLockInstruction({ lockId: "merchant-lock-001", expectedRemainingAmount: "15" });',
        'const bareCancelAssetLock: CancelAssetLockV1 = { escrow_id: "hash:73CCD4E0DD69AD434DB75056B600AA4F74C8FC5556B11BDC799DFDB7EA29851F#434B", expected_remaining_amount: "20" };',
        "const bareCancelAssetLockArchive: CancelAssetLockV1Archive = encodeCancelAssetLockV1(bareCancelAssetLock);",
        "const decodedBareCancelAssetLock: CancelAssetLockV1 = decodeCancelAssetLockV1(bareCancelAssetLockArchive);",
        "const appealFinanceOutcome: SorafsValidationOutcome = validateAppealFinanceCancelAssetLock(bareCancelAssetLockArchive, { label: 'cancel_asset_lock_v1.to', generatedAtUnix: 41n });",
        "// @ts-expect-error bare V1 quantities must be canonical strings.",
        'encodeCancelAssetLockV1({ escrow_id: bareCancelAssetLock.escrow_id, expected_remaining_amount: 20 });',
        "// @ts-expect-error quantity-bearing APIs reject lossy JavaScript numbers.",
        'buildCancelAssetLockInstruction({ lockId: "merchant-lock-001", expectedRemainingAmount: 15 });',
        'const availability: AssetTransferAvailability = "Disabled";',
        'const setAvailability: SetAssetTransferAvailabilityInstruction = buildSetAssetTransferAvailabilityInstruction({ accountId: "i105...", assetDefinitionId: "asset...", expectedRevision: 0, incoming: availability, outgoing: "Enabled" });',
        '// @ts-expect-error availability spellings are exact.',
        'buildSetAssetTransferAvailabilityInstruction({ accountId: "i105...", assetDefinitionId: "asset...", expectedRevision: 0, incoming: "disabled", outgoing: "Enabled" });',
        "const toriiConstructor: typeof ToriiClient = Torii.ToriiClient;",
        "declare const signedOrderbookTransaction: Uint8Array;",
        "const orderbookReceipt: Promise<SorafsOrderbookSubmissionReceipt> = new ToriiClient('https://torii.example').submitSorafsOrderbookOrder(signedOrderbookTransaction, { expectedReceiptSigner: 'ed0120...' });",
        "const orderbookAmbiguity: typeof SorafsOrderbookSubmissionAmbiguousError = Torii.SorafsOrderbookSubmissionAmbiguousError;",
        "// @ts-expect-error the receipt trust anchor is mandatory.",
        "new ToriiClient('https://torii.example').submitSorafsOrderbookOrder(signedOrderbookTransaction);",
        "// @ts-expect-error signed orderbook wires reject text inputs.",
        "new ToriiClient('https://torii.example').submitSorafsOrderbookOrder('wire', { expectedReceiptSigner: 'ed0120...' });",
        "// @ts-expect-error signed orderbook wires reject numeric arrays.",
        "new ToriiClient('https://torii.example').submitSorafsOrderbookCancel([1, 2, 3], { expectedReceiptSigner: 'ed0120...' });",
        `const encodeInstruction: typeof export${noritoIndex}.noritoEncodeInstruction = Norito.noritoEncodeInstruction;`,
        `const validateFrame: typeof export${noritoIndex}.validateNoritoFrame = Norito.validateNoritoFrame;`,
        `const exact12Decoder: typeof export${noritoIndex}.noritoDecodePrivacyExact12FixtureBundleBase64V1 = Norito.noritoDecodePrivacyExact12FixtureBundleBase64V1;`,
        "// @ts-expect-error fixture-only Exact12 codecs are not retained by the broad browser facade.",
        `void export${browserIndex}.noritoDecodePrivacyExact12FixtureBundleBase64V1;`,
        `const generateKeyPair: typeof export${cryptoIndex}.generateKeyPair = Crypto.generateKeyPair;`,
        "const privacySnapshot: PrivacyCapabilitySnapshotV1 = parsePrivacyCapabilitySnapshotV1({});",
        "const privacyCommittedHeight: bigint = privacySnapshot.committed_height;",
        "const privacyNodeResult: Promise<PrivacyCapabilitySnapshotV1> = getPrivacyCapabilitiesV1(new ToriiClient('https://torii.example'), { canonicalAuth: { accountId: 'i105...', privateKey: '11'.repeat(32) } });",
        "const privacyBrowserResult: Promise<PrivacyCapabilitySnapshotV1> = getPrivacyCapabilitiesV1(new ToriiBrowserClient('https://torii.example'), { authAccountId: 'i105...', sign: async () => new Uint8Array(64) });",
        'const privacyProofSystems: PrivacyProofSystemIdV1[] = ["stark-fri-sha256-goldilocks", "anonymous-pgc-p256", "iroha-verange-p256", "zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512", "vega-neutron-nova-spartan-hyrax-t256", "jindo-polynomial-commitment", "lantern-lnp22-module-linear-norm", "halo2-ipa-pasta", "fcmp-plus-plus-curve-tree-bulletproofs"];',
        'const privacyEngines: PrivacyEngineIdV1[] = ["native-goldilocks-stark-fri", "native-anonymous-pgc-p256", "native-verange-p256", "native-zk-ams-masked-relaxed-spartan-t256-ristretto255", "native-vega", "native-jindo", "native-lantern-lnp22", "native-halo2-orchard", "native-fcmp-plus-plus"];',
        "// @ts-expect-error retired SIS-with-hints proof systems fail closed.",
        'const retiredPrivacyProofSystem: PrivacyProofSystemIdV1 = "sis-with-hints";',
        "// @ts-expect-error proof-system labels are case-sensitive.",
        'const caseShiftedPrivacyProofSystem: PrivacyProofSystemIdV1 = "HALO2-IPA-PASTA";',
        "// @ts-expect-error proof-system labels reject surrounding whitespace.",
        'const paddedPrivacyProofSystem: PrivacyProofSystemIdV1 = " halo2-ipa-pasta";',
        "// @ts-expect-error retired SIS-with-hints engines fail closed.",
        'const retiredPrivacyEngine: PrivacyEngineIdV1 = "native-sis-with-hints";',
        "// @ts-expect-error engine labels are case-sensitive.",
        'const caseShiftedPrivacyEngine: PrivacyEngineIdV1 = "NATIVE-JINDO";',
        "// @ts-expect-error engine labels reject Unicode confusables.",
        'const confusablePrivacyEngine: PrivacyEngineIdV1 = "native-jindо";',
        "declare const repoAgreement: ToriiRepoAgreement;",
        "const repoLifecycle: [string, string, number | null, 'active' | 'settled'] = [repoAgreement.cashSource, repoAgreement.collateralCustodyAsset, repoAgreement.settlementTimestampMs, repoAgreement.status];",
        'const verifierBackend: ToriiVerifierBackendLabelV1 = "halo2/ipa";',
        "// @ts-expect-error retired privacy backend aliases fail closed.",
        'const retiredVerifierBackend: ToriiVerifierBackendLabelV1 = "halo2/ipa-pasta-cycle-v1";',
        "// @ts-expect-error backend labels are case-sensitive.",
        'const caseShiftedVerifierBackend: ToriiVerifierBackendLabelV1 = "HALO2/IPA";',
        "// @ts-expect-error backend labels reject surrounding whitespace.",
        'const paddedVerifierBackend: ToriiVerifierBackendLabelV1 = " halo2/ipa";',
        "// @ts-expect-error backend labels reject Unicode confusables.",
        'const confusableVerifierBackend: ToriiVerifierBackendLabelV1 = "halо2/ipa";',
        "// @ts-expect-error privacy capability fetch is not a base-client method.",
        "void ToriiClient.prototype.getPrivacyCapabilitiesV1;",
        "// @ts-expect-error privacy capability parser is not a root runtime export.",
        "void RootSdk.parsePrivacyCapabilitySnapshotV1;",
        "// @ts-expect-error the optional fetch API rejects unknown request options.",
        "getPrivacyCapabilitiesV1(new ToriiClient('https://torii.example'), { canonicalAuth: { accountId: 'i105...', privateKey: '11'.repeat(32) }, unknown: true });",
        "const quantityFrame: Uint8Array = NumericV1.encodeQuantityFrame(42n);",
        "const quantityEnvelope: Uint8Array = NumericV1.encodeQuantityEnvelope(42n);",
        "const quantityJson: string = NumericV1.encodeQuantityJson(42n);",
        'const rootNumericKinds: ContractEntrypointValueKindName[] = ["Int", "Decimal", "Quantity"];',
        '// @ts-expect-error Amount is a permanently retired V1 boundary kind.',
        'const retiredRootAmount: ContractEntrypointValueKindName = "Amount";',
        '// @ts-expect-error U128 is not a canonical V1 boundary kind.',
        'const retiredRootU128: ContractEntrypointValueKindName = "U128";',
        "async function checkIdentifierApiTypes(client: ToriiClient, auth: CanonicalRequestAuth): Promise<void> {",
        "  const policies: IdentifierPolicyListResponse = await client.listIdentifierPolicies();",
        "  const policy = policies.items[0]!;",
        "  const execution: RamLfeExecuteResponse | null = await client.executeRamLfeProgram(policy.program_id, { encryptedInput: 'ABCD', canonicalAuth: auth });",
        "  const claim: IdentifierClaimLookupResponse | null = await client.getIdentifierClaimByReceiptHash('11'.repeat(32));",
        "  if (execution) {",
        "    const opening: RamLfeOutputOpening = execution.output_opening;",
        "    const resolved: IdentifierResolutionReceipt | null = await client.resolveIdentifier({ policyId: policy.policy_id, encryptedInput: 'ABCD', outputOpening: opening, canonicalAuth: auth });",
        "    const issued: IdentifierResolutionReceipt | null = await client.issueIdentifierClaimReceipt('account-id', { policyId: policy.policy_id, encryptedInput: 'ABCD', outputOpening: opening, canonicalAuth: auth });",
        "    void [resolved, issued, opening.payload.opened_output_hash];",
        "  }",
        "  void claim;",
        "}",
        "// @ts-expect-error Torii does not expose crypto helpers.",
        "void Torii.generateKeyPair;",
        "// @ts-expect-error Crypto does not expose Torii clients.",
        "void Crypto.ToriiClient;",
        "// @ts-expect-error Norito does not expose crypto helpers.",
        "void Norito.generateKeyPair;",
        `void [${bindings.join(", ")}];`,
        "void algorithm; void cancelAssetLock; void toriiConstructor; void orderbookReceipt; void orderbookAmbiguity; void encodeInstruction; void validateFrame; void exact12Decoder; void generateKeyPair; void privacySnapshot; void privacyNodeResult; void privacyBrowserResult; void privacyProofSystems; void privacyEngines; void retiredPrivacyProofSystem; void caseShiftedPrivacyProofSystem; void paddedPrivacyProofSystem; void retiredPrivacyEngine; void caseShiftedPrivacyEngine; void confusablePrivacyEngine; void repoLifecycle; void verifierBackend; void retiredVerifierBackend; void caseShiftedVerifierBackend; void paddedVerifierBackend; void confusableVerifierBackend; void quantityFrame; void quantityEnvelope; void quantityJson; void rootNumericKinds; void retiredRootAmount; void retiredRootU128; void checkIdentifierApiTypes;",
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
  for (const declaration of ["index.d.ts", "src/blockProofTypes.d.ts"]) {
    const source = fs.readFileSync(path.join(PACKAGE_ROOT, declaration), "utf8");
    assert.doesNotMatch(source, /reference types=["']node["']/u, declaration);
    assert.doesNotMatch(source, /["']node:/u, declaration);
  }
  const { tempRoot } = createPackedLayout({ includeNodeTypes: false });
  try {
    assert.equal(fs.existsSync(path.join(tempRoot, "node_modules", "@types", "node")), false);
    fs.writeFileSync(
      path.join(tempRoot, "browser-consumer.ts"),
      [
        'import { createConnectAppSession, createConnectCanonicalRequestAuth, TORII_CANONICAL_REQUEST_DOMAIN_TAG, type BrowserConnectApproval, type BrowserConnectCanonicalRequestAuth } from "@iroha/iroha-js/connect-browser";',
        'import { NexusAppClient, type NexusFinalizeOptions } from "@iroha/iroha-js/nexus-app";',
        'import { browserTransactionCodec } from "@iroha/iroha-js/transaction-codec";',
        'import { computeIvmArtifactHashes } from "@iroha/iroha-js/ivm-artifact";',
        'import { canonicalQueryString } from "@iroha/iroha-js/canonical-request";',
        'import { getPrivacyCapabilitiesV1, type PrivacyCapabilitiesBrowserClientV1, type PrivacyCapabilitySnapshotV1 } from "@iroha/iroha-js/privacy-capabilities";',
        'import { compileKotodamaProgram, KotodamaCompilerClient, type KotodamaCompiledEntrypointValueKindName, type KotodamaCompiledManifestMetadata, type KotodamaCompilerCallOptions, type KotodamaCompilerTransportOptions } from "@iroha/iroha-js/kotodama-compiler";',
        "declare const approval: BrowserConnectApproval;",
        "declare const connectSession: ReturnType<typeof createConnectAppSession>;",
        "const connectAuth: Promise<BrowserConnectCanonicalRequestAuth> = createConnectCanonicalRequestAuth(connectSession);",
        'const options: NexusFinalizeOptions = { wait: true, signal: new AbortController().signal };',
        "const bytes = new Uint8Array(17);",
        "void createConnectAppSession; void approval; void connectAuth; void options;",
        "void connectSession.signRaw(TORII_CANONICAL_REQUEST_DOMAIN_TAG, bytes);",
        "void new NexusAppClient({ chainDiscriminant: 753, transactionCodec: browserTransactionCodec });",
        "void computeIvmArtifactHashes(bytes);",
        "declare const privacyClient: PrivacyCapabilitiesBrowserClientV1;",
        "const privacyResult: Promise<PrivacyCapabilitySnapshotV1> = getPrivacyCapabilitiesV1(privacyClient, { headers: { Accept: 'application/json' }, authAccountId: 'i105...', sign: async () => new Uint8Array(64) });",
        'void canonicalQueryString(new URLSearchParams({ browser: "true" }));',
        'void compileKotodamaProgram("CREATE DOMAIN browser");',
        "const compilerTransport: KotodamaCompilerTransportOptions = { signal: new AbortController().signal, timeoutMs: 30_000 };",
        "const compilerCall: KotodamaCompilerCallOptions = { ...compilerTransport, sourceName: 'browser.ko', zk: true };",
        "void new KotodamaCompilerClient('https://compiler.example').compile('seiyaku Demo {}', compilerCall);",
        'const compilerNumericKinds: KotodamaCompiledEntrypointValueKindName[] = ["Int", "Decimal", "Quantity"];',
        "declare const compilerManifest: KotodamaCompiledManifestMetadata;",
        "const compilerManifestName: string = compilerManifest.seiyaku_name;",
        "const compilerFingerprint: string = compilerManifest.compiler_fingerprint;",
        "const compilerFeatureBitmap: number = compilerManifest.features_bitmap;",
        "const compilerEntrypoints = compilerManifest.entrypoints.map(({ name }) => name);",
        "const compilerStates = compilerManifest.states.map(({ name }) => name);",
        '// @ts-expect-error Amount is a permanently retired V1 boundary kind.',
        'const retiredCompilerAmount: KotodamaCompiledEntrypointValueKindName = "Amount";',
        '// @ts-expect-error U128 is not a canonical V1 boundary kind.',
        'const retiredCompilerU128: KotodamaCompiledEntrypointValueKindName = "U128";',
        'void privacyResult; void compilerTransport; void compilerCall; void compilerNumericKinds; void compilerManifestName; void compilerFingerprint; void compilerFeatureBitmap; void compilerEntrypoints; void compilerStates; void retiredCompilerAmount; void retiredCompilerU128;',
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
