"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import { AccountAddress } from "../dist/index.js";
import { compileKotodamaProgram as compileDistKotodamaProgram } from "../dist/kotodamaCompiler/index.js";
import { renderCanonicalAccountIdLiteralFromPublicKeyLiteral } from "../src/kotodamaCompiler/accountLiteral.js";
import { compileKotodamaProgram as compileSrcKotodamaProgram } from "../src/kotodamaCompiler/index.js";

const LEGACY_FULLWIDTH_KANA = /[イロハニホヘトチリヌルヲワカヨタレソツネナラムウノオクヤマケフコエテアサキユメミシヒモセス]/u;
const HALFWIDTH_KANA = /[ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼﾋﾓｾｽ]/u;

test("package dist entrypoint imports and emits halfwidth i105 literals", () => {
  const publicKey = Buffer.from(
    "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20",
    "hex",
  );
  const address = AccountAddress.fromAccount({ publicKey });
  const literal = address.toI105(0x02f1);

  assert.match(literal, /^sora/u);
  assert.equal(LEGACY_FULLWIDTH_KANA.test(literal), false);
  assert.equal(HALFWIDTH_KANA.test(literal), true);
});

test("package dist Kotodama compiler rejects AssetDefinitionId checksum mismatches", () => {
  const result = compileDistKotodamaProgram(`
seiyaku BadAssetDefinitionChecksum {
  kotoage fn run() permission(Admin) {
    mint_asset(authority(), asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjN"), 1);
  }
}
`);

  assert.equal(result.artifactBytes.length, 0);
  assert.equal(result.diagnostics.length, 1);
  assert.match(result.diagnostics[0].message, /invalid AssetDefinitionId literal `62Fk4FPcMuLvW5QjDGNF2a4jAmjN`.*checksum/is);
});

test("package dist Kotodama compiler matches src for direct account mint path", () => {
  const account = renderCanonicalAccountIdLiteralFromPublicKeyLiteral(`ed0120${"11".repeat(32)}`);
  const asset = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
  const source = `
seiyaku DirectAccountMint {
  kotoage fn run() permission(Admin) {
    mint_asset(account_id("${account}"), asset_definition("${asset}"), 1);
  }
}
`;

  const srcResult = compileSrcKotodamaProgram(source);
  const distResult = compileDistKotodamaProgram(source);

  assert.deepEqual(srcResult.diagnostics, []);
  assert.deepEqual(distResult.diagnostics, []);
  assert.deepEqual(Buffer.from(distResult.artifactBytes), Buffer.from(srcResult.artifactBytes));
});
