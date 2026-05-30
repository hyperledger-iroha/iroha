import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  compileKotodamaProgram,
  compileKotodamaStudioProgram,
  normalizeKotodamaParitySource,
} from "../src/kotodamaCompiler/index.js";
import { renderCanonicalAccountIdLiteralFromPublicKeyLiteral } from "../src/kotodamaCompiler/accountLiteral.js";
import { blake2b256 } from "../src/blake2b.js";

const CURRENT_ABI_V1_HASH_HEX = "73cefb1b419f97b9e2864cdc6545d3f80ae2328dc0fbe2fbd034cd51a837ba0d";
const CURRENT_ABI_V1_HASH_LITERAL =
  "hash:73CEFB1B419F97B9E2864CDC6545D3F80AE2328DC0FBE2FBD034CD51A837BA0D#4D00";

function irohaHashHex(bytes) {
  const digest = Buffer.from(blake2b256(bytes));
  if (digest.length > 0) {
    digest[digest.length - 1] |= 1;
  }
  return digest.toString("hex");
}

function readArtifactSection(artifactBytes, offset) {
  const artifact = Buffer.from(artifactBytes);
  const tag = artifact.subarray(offset, offset + 4).toString("ascii");
  const length = artifact.readUInt32LE(offset + 4);
  const payloadStart = offset + 8;
  const end = payloadStart + length;

  return {
    tag,
    length,
    payloadStart,
    end,
    payload: artifact.subarray(payloadStart, end),
  };
}

function readArtifactCode(artifactBytes) {
  const artifact = Buffer.from(artifactBytes);
  let offset = 17;
  while (offset + 8 <= artifact.length) {
    const tag = artifact.subarray(offset, offset + 4).toString("ascii");
    if (tag === "CNTR" || tag === "DBG1") {
      offset += 8 + artifact.readUInt32LE(offset + 4);
      continue;
    }
    if (tag === "LTLB") {
      const count = artifact.readUInt32LE(offset + 4);
      const postPad = artifact.readUInt32LE(offset + 8);
      const dataLength = artifact.readUInt32LE(offset + 12);
      offset += 16 + count * 8 + dataLength + postPad;
      continue;
    }
    if (tag !== "CNTR" && tag !== "DBG1" && tag !== "LTLB") {
      break;
    }
  }
  return artifact.subarray(offset);
}

function readLiteralTableEntries(artifactBytes) {
  const artifact = Buffer.from(artifactBytes);
  let offset = 17;

  while (offset + 8 <= artifact.length) {
    const tag = artifact.subarray(offset, offset + 4).toString("ascii");
    if (tag === "CNTR" || tag === "DBG1") {
      offset += 8 + artifact.readUInt32LE(offset + 4);
      continue;
    }
    if (tag !== "LTLB") {
      return [];
    }

    const count = artifact.readUInt32LE(offset + 4);
    const dataLength = artifact.readUInt32LE(offset + 12);
    const dataStart = offset + 16 + count * 8;
    const data = artifact.subarray(dataStart, dataStart + dataLength);
    const entries = [];
    let cursor = 0;
    while (cursor < data.length) {
      const type = data.readUInt16BE(cursor);
      const version = data[cursor + 2];
      const length = data.readUInt32BE(cursor + 3);
      const payloadStart = cursor + 7;
      const payload = data.subarray(payloadStart, payloadStart + length);
      entries.push({ type, version, length, payload });
      cursor = payloadStart + length + 32;
    }
    return entries;
  }

  return [];
}

function noritoPayloadBody(payload) {
  assert.equal(payload.subarray(0, 4).toString("ascii"), "NRT0");
  assert.equal(payload[39], 0x02);
  return payload.subarray(40);
}

function compactBitmapU32Needle(mask) {
  const bytes = Buffer.alloc(12);
  bytes.writeBigUInt64LE(4n, 0);
  bytes.writeUInt32LE(mask >>> 0, 8);
  return bytes;
}

function syscallNeedle(syscall) {
  return Buffer.from([syscall, 0x00, 0x00, 0x60]);
}

function syscallxNeedle(syscall) {
  const word = ((0x62 << 24) | syscall) >>> 0;
  const bytes = Buffer.alloc(4);
  bytes.writeUInt32LE(word);
  return bytes;
}

function ivmWord(op, a, b, c) {
  return (((op & 0xff) << 24) | ((a & 0xff) << 16) | ((b & 0xff) << 8) | (c & 0xff)) >>> 0;
}

function ivmWordNeedle(word) {
  const bytes = Buffer.alloc(4);
  bytes.writeUInt32LE(word >>> 0);
  return bytes;
}

function invokeEntrypointAsFlagNeedle(flag) {
  return Buffer.concat([
    ivmWordNeedle(ivmWord(0x20, 13, 0, flag)),
    syscallxNeedle(0x00fe_0004),
  ]);
}

test("Kotodama compiler SDK compiles a minimal contract artifact", () => {
  const compiled = compileKotodamaStudioProgram(`
seiyaku SdkSmoke {
  hajimari() {
    info("ready");
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(compiled.abiHashHex.length, 64);
  assert.equal(compiled.abiHashHex, CURRENT_ABI_V1_HASH_HEX);
  assert.equal(compiled.codeHashHex.length, 64);
  assert.equal(compiled.codeHashHex, irohaHashHex(compiled.artifactBytes.slice(17)));
  assert.ok(compiled.artifactBytes.length > 32);
  assert.equal(compiled.manifest?.abi_hash, CURRENT_ABI_V1_HASH_LITERAL);
  assert.equal(compiled.manifest?.compiler_fingerprint, compiled.compilerFingerprint);
  assert.equal(compiled.sourceMap[0]?.function_name, "hajimari");
});

test("Kotodama compiler SDK exposes the reusable compile alias", () => {
  assert.equal(compileKotodamaProgram, compileKotodamaStudioProgram);
});

test("Kotodama compiler SDK mirrors Rust no-function diagnostics", () => {
  const empty = compileKotodamaProgram("");
  const onlyStruct = compileKotodamaProgram("struct User { value: int }");
  const onlyProductionStrippedTest = compileKotodamaProgram(`
#[test]
fn smoke() {}
`);

  assert.equal(empty.artifactBytes.length, 0);
  assert.equal(empty.diagnostics[0].message, "no functions to compile");
  assert.equal(onlyStruct.artifactBytes.length, 0);
  assert.equal(onlyStruct.diagnostics[0].message, "no functions to compile");
  assert.equal(onlyProductionStrippedTest.artifactBytes.length, 0);
  assert.equal(onlyProductionStrippedTest.diagnostics[0].message, "no functions to compile");
});

test("Kotodama compiler SDK accepts upstream loose top-level declarations", () => {
  const internalMain = compileKotodamaProgram(`
fn main() -> int {
  return 7;
}
`);
  const looseContract = compileKotodamaProgram(`
kotoba { "E1": { en: "bad quote" } }
const BONUS: int = 2;
struct Pair { a: int; b: int; }
state total: int;

hajimari() {
  total = 1;
}

kotoage fn main() -> int permission(Admin) {
  let pair = Pair(BONUS, 3);
  total = total + pair.a;
  return total;
}
`);
  const triggerCallback = compileKotodamaProgram(`
kotoage fn run() {
  let event = trigger_event();
  let kind = event.get_name(name("kind"));
  info(1);
}

register_trigger wake {
  call run;
  on execute trigger wake;
}
`);

  assert.deepEqual(internalMain.diagnostics, []);
  assert.equal(internalMain.artifactBytes.length > 32, true);
  assert.equal(internalMain.manifest?.entrypoints.length, 1);
  assert.equal(internalMain.manifest?.entrypoints[0]?.name, "main");
  assert.deepEqual(internalMain.manifest?.entrypoints[0]?.kind, { kind: "Public", value: null });
  assert.equal(internalMain.manifest?.entrypoints[0]?.return_type, "int");
  assert.equal(internalMain.manifest?.entrypoints[0]?.access_hints_complete, null);
  assert.deepEqual(looseContract.diagnostics, []);
  assert.equal(looseContract.artifactBytes.length > 32, true);
  assert.equal(looseContract.manifest?.entrypoints.length, 2);
  assert.equal(looseContract.manifest?.kotoba?.length, 1);
  assert.deepEqual(triggerCallback.diagnostics, []);
  assert.equal(triggerCallback.manifest?.entrypoints[0]?.name, "run");
});

test("Kotodama compiler SDK accepts kaizen upgrade hooks", () => {
  const compiled = compileKotodamaProgram(`
seiyaku UpgradeHook {
  kaizen() {
    info("upgrade");
  }
}
`);
  const unicode = compileKotodamaProgram(`
seiyaku UpgradeHookUnicode {
  改善() {
    info("upgrade");
  }
}
`);
  const withParamPermission = compileKotodamaProgram(`
seiyaku UpgradeHookParam {
  kaizen(_new_impl: int) permission(Admin) {
    info("upgrade requested");
  }
}
`);

  const cntrBody = noritoPayloadBody(readArtifactSection(compiled.artifactBytes, 17).payload);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(compiled.manifest?.entrypoints.length, 1);
  assert.equal(compiled.manifest?.entrypoints[0]?.name, "kaizen");
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.kind, { kind: "Kaizen", value: null });
  assert.notEqual(cntrBody.indexOf(Buffer.from([
    ...Buffer.from("kaizen", "utf8"),
    0x04, 0x03, 0x00, 0x00, 0x00,
  ])), -1);
  assert.deepEqual(unicode.diagnostics, []);
  assert.equal(unicode.manifest?.entrypoints[0]?.name, "kaizen");
  assert.deepEqual(unicode.manifest?.entrypoints[0]?.kind, { kind: "Kaizen", value: null });
  assert.deepEqual(withParamPermission.diagnostics, []);
  assert.equal(withParamPermission.manifest?.entrypoints[0]?.name, "kaizen");
  assert.deepEqual(withParamPermission.manifest?.entrypoints[0]?.params, [
    { name: "_new_impl", type_name: "int" },
  ]);
  assert.equal(withParamPermission.manifest?.entrypoints[0]?.permission, "Admin");
});

test("Kotodama compiler SDK compiles info through the generic call path", () => {
  const valid = compileKotodamaProgram(`
seiyaku InfoCall {
  fn helper() {
    call info("ready");
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalid = compileKotodamaProgram(`
seiyaku InvalidInfoCall {
  fn helper() {
    call info(json!{ value: "ready" });
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const code = readArtifactCode(valid.artifactBytes);

  assert.deepEqual(valid.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x03)), -1, "missing DEBUG_LOG syscall");
  assert.equal(invalid.artifactBytes.length, 0);
  assert.equal(invalid.diagnostics.length, 1);
  assert.equal(invalid.diagnostics[0].message, "semantic error: info expects (string|int)");
});

test("Kotodama compiler SDK validates bare info arguments like Rust", () => {
  const numeric = compileKotodamaProgram(`
seiyaku InfoNumeric {
  kotoage fn run(amount: Amount) permission(Admin) {
    info(amount);
    call info(amount);
  }
}
`);
  const pointer = compileKotodamaProgram(`
fn helper() {
  let event = trigger_event();
  info(event);
}
`);

  assert.deepEqual(numeric.diagnostics, []);
  assert.notEqual(readArtifactCode(numeric.artifactBytes).indexOf(syscallNeedle(0x03)), -1);
  assert.equal(pointer.artifactBytes.length, 0);
  assert.equal(pointer.diagnostics.length, 1);
  assert.equal(pointer.diagnostics[0].message, "semantic error: info expects (string|int)");
});

test("Kotodama compiler SDK rejects trigger payload helpers in public entrypoints", () => {
  const compiled = compileKotodamaProgram(`
seiyaku PayloadGuard {
  kotoage fn run() {
    let event = trigger_event();
    info(1);
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(
    compiled.diagnostics[0].message,
    /public and view entrypoints cannot use `trigger_event` here/,
  );
});

test("Kotodama compiler SDK rejects trigger payload helpers in view entrypoints", () => {
  const compiled = compileKotodamaProgram(`
seiyaku PayloadViewGuard {
  view fn amount() -> int {
    let payload = json!{ amount: 7 };
    return payload.get_int(name("amount"));
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(
    compiled.diagnostics[0].message,
    /public and view entrypoints cannot use `get_int` here/,
  );
});

test("Kotodama compiler SDK rejects mutating map ensure in view entrypoints", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ViewEnsureGuard {
  view fn amount() -> int {
    let balances: Map<int, int> = Map::new();
    return balances.ensure(7, 9);
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(
    compiled.diagnostics[0].message,
    /view entrypoints cannot use mutating map helper `ensure`/,
  );
});

test("Kotodama compiler SDK allows non-mutating map get_or in view entrypoints", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ViewGetOr {
  view fn amount() -> int {
    let balances: Map<int, int> = Map::new();
    return balances.get_or(7, 9);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(compiled.artifactBytes.length > 0);
});

test("Kotodama compiler SDK allows non-mutating map get_or_default in view entrypoints", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ViewGetOrDefault {
  view fn amount() -> int {
    let balances: Map<int, int> = Map::new();
    return get_or_default(balances, 7, 9);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(compiled.artifactBytes.length > 0);
});

test("Kotodama compiler SDK rejects direct durable state mutation in view entrypoints", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ViewStateMutation {
  state int Balance;

  view fn amount() -> int {
    Balance = 1;
    return Balance;
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(
    compiled.diagnostics[0].message,
    /view function `amount` cannot perform durable state mutation/,
  );
});

test("Kotodama compiler SDK rejects direct host side effects in view entrypoints", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ViewHostEffect {
  view fn move_tokens(from: AccountId, to: AccountId, asset: AssetDefinitionId) -> int {
    transfer_asset(from, to, asset, 1);
    return 1;
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(
    compiled.diagnostics[0].message,
    /view function `move_tokens` cannot perform host side effects/,
  );
});

test("Kotodama compiler SDK rejects native escrow side effects in view entrypoints", () => {
  const direct = compileKotodamaProgram(`
seiyaku ViewEscrowEffect {
  view fn inspect() -> int {
    escrow_accept(name("aitai_offer"));
    return 1;
  }
}
`);
  const transitive = compileKotodamaProgram(`
seiyaku ViewEscrowHelperEffect {
  fn release() {
    escrow_release(name("aitai_offer"));
  }

  view fn inspect() -> int {
    release();
    return 1;
  }
}
`);

  assert.equal(direct.artifactBytes.length, 0);
  assert.equal(direct.diagnostics.length, 1);
  assert.match(
    direct.diagnostics[0].message,
    /view function `inspect` cannot perform host side effects/,
  );
  assert.equal(transitive.artifactBytes.length, 0);
  assert.equal(transitive.diagnostics.length, 1);
  assert.match(
    transitive.diagnostics[0].message,
    /view function `inspect` cannot call `release` because `release` performs host side effects/,
  );
});

test("Kotodama compiler SDK rejects transitive instruction emission in view entrypoints", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ViewInstructionEffect {
  fn helper() {
    execute_instruction(norito_bytes("0x0102"));
  }

  view fn inspect() -> int {
    helper();
    return 1;
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(
    compiled.diagnostics[0].message,
    /view function `inspect` cannot call `helper` because `helper` performs instruction emission/,
  );
});

test("Kotodama compiler SDK requires permission for public host side effects", () => {
  const compiled = compileKotodamaProgram(`
seiyaku PublicHostEffect {
  kotoage fn move_tokens(from: AccountId, to: AccountId, asset: AssetDefinitionId) {
    transfer_asset(from, to, asset, 1);
  }
}
`);
  const batchCompiled = compileKotodamaProgram(`
seiyaku PublicBatchHostEffect {
  kotoage fn batch() {
    transfer_v1_batch_begin();
    transfer_v1_batch_end();
  }
}
`);
  const callCompiled = compileKotodamaProgram(`
seiyaku PublicCallHostEffect {
  kotoage fn move_tokens(from: AccountId, to: AccountId, asset: AssetDefinitionId) {
    call transfer_asset(from, to, asset, 1);
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(
    compiled.diagnostics[0].message,
    /public function `move_tokens` calls privileged operations but is missing `permission\(\.\.\.\)`/,
  );
  assert.equal(batchCompiled.artifactBytes.length, 0);
  assert.equal(batchCompiled.diagnostics.length, 1);
  assert.match(
    batchCompiled.diagnostics[0].message,
    /public function `batch` calls privileged operations but is missing `permission\(\.\.\.\)`/,
  );
  assert.equal(callCompiled.artifactBytes.length, 0);
  assert.equal(callCompiled.diagnostics.length, 1);
  assert.match(
    callCompiled.diagnostics[0].message,
    /public function `move_tokens` calls privileged operations but is missing `permission\(\.\.\.\)`/,
  );
});

test("Kotodama compiler SDK requires permission for transitive public instruction emission", () => {
  const compiled = compileKotodamaProgram(`
seiyaku PublicInstructionEffect {
  fn helper() {
    execute_instruction(norito_bytes("0x0102"));
  }

  kotoage fn run() {
    helper();
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(
    compiled.diagnostics[0].message,
    /public function `run` calls privileged operations but is missing `permission\(\.\.\.\)`/,
  );
});

test("Kotodama compiler SDK requires permission for public native escrow side effects", () => {
  const direct = compileKotodamaProgram(`
seiyaku PublicEscrowPermission {
  kotoage fn run() {
    escrow_cancel(name("aitai_offer"));
  }
}
`);
  const transitive = compileKotodamaProgram(`
seiyaku PublicEscrowHelperPermission {
  fn cancel() {
    escrow_cancel(name("aitai_offer"));
  }

  kotoage fn run() {
    cancel();
  }
}
`);

  assert.equal(direct.artifactBytes.length, 0);
  assert.equal(direct.diagnostics.length, 1);
  assert.match(
    direct.diagnostics[0].message,
    /public function `run` calls privileged operations but is missing `permission\(\.\.\.\)`/,
  );
  assert.equal(transitive.artifactBytes.length, 0);
  assert.equal(transitive.diagnostics.length, 1);
  assert.match(
    transitive.diagnostics[0].message,
    /public function `run` calls privileged operations but is missing `permission\(\.\.\.\)`/,
  );
});

test("Kotodama compiler SDK accepts public host side effects with permission", () => {
  const compiled = compileKotodamaProgram(`
seiyaku PublicHostEffectAllowed {
  kotoage fn move_tokens(from: AccountId, to: AccountId, asset: AssetDefinitionId) permission(Admin) {
    transfer_asset(from, to, asset, 1);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(compiled.artifactBytes.length > 0);
});

test("Kotodama compiler SDK accepts upstream permission modifiers on non-public functions", () => {
  const helperPermission = compileKotodamaProgram(`
seiyaku HelperPermissionModifier {
  fn helper() permission(Admin) {}

  kotoage fn run() permission(Admin) {
    helper();
  }
}
`);
  const viewPermission = compileKotodamaProgram(`
seiyaku ViewPermissionModifier {
  view fn inspect() permission(Admin) {}

  kotoage fn run() permission(Admin) {}
}
`);
  const duplicateHelper = compileKotodamaProgram(`
seiyaku DuplicateHelperPermission {
  fn helper() permission(Admin) permission(User) {}

  kotoage fn run() permission(Admin) {
    helper();
  }
}
`);
  const duplicatePublic = compileKotodamaProgram(`
seiyaku DuplicatePublicPermission {
  kotoage fn run() permission(Admin) permission(User) {}
}
`);

  assert.deepEqual(helperPermission.diagnostics, []);
  assert.ok(helperPermission.artifactBytes.length > 0);
  assert.deepEqual(viewPermission.diagnostics, []);
  assert.ok(viewPermission.artifactBytes.length > 0);
  assert.equal(duplicateHelper.artifactBytes.length, 0);
  assert.equal(duplicateHelper.diagnostics.length, 1);
  assert.match(duplicateHelper.diagnostics[0].message, /parser error: duplicate permission modifier/);
  assert.equal(duplicatePublic.artifactBytes.length, 0);
  assert.equal(duplicatePublic.diagnostics.length, 1);
  assert.match(duplicatePublic.diagnostics[0].message, /parser error: duplicate permission modifier/);
});

test("Kotodama compiler SDK rejects state parameters on public entrypoints", () => {
  const compiled = compileKotodamaProgram(`
seiyaku PublicStateParam {
  kotoage fn run(state Map<Name, int> balances) -> int {
    return 1;
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(
    compiled.diagnostics[0].message,
    /state parameter `balances` is only supported on internal helper functions/,
  );
});

test("Kotodama compiler SDK rejects locals that shadow state declarations", () => {
  const compiled = compileKotodamaProgram(`
seiyaku StateShadowLet {
  state int counter;

  fn helper() {
    let counter = 1;
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(compiled.diagnostics[0].message, /E_STATE_SHADOWED/);
});

test("Kotodama compiler SDK rejects params that shadow state declarations", () => {
  const compiled = compileKotodamaProgram(`
seiyaku StateShadowParam {
  state int counter;

  fn helper(counter: int) {}

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(compiled.diagnostics[0].message, /E_STATE_SHADOWED/);
});

test("Kotodama compiler SDK requires durable handles for state parameters", () => {
  const compiled = compileKotodamaProgram(`
seiyaku StateParamHandle {
  state Balances: Map<Name, int>;

  fn ensure_balance(state Map<Name, int> balances, key: Name) -> int {
    return balances.ensure(key, 0);
  }

  kotoage fn run(key: Name) -> int permission(Admin) {
    let balances: Map<Name, int> = Map::new();
    return ensure_balance(balances, key);
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(
    compiled.diagnostics[0].message,
    /state parameter `balances` requires a durable state handle argument/,
  );
});

test("Kotodama compiler SDK accepts durable handles for state parameters", () => {
  const compiled = compileKotodamaProgram(`
seiyaku StateParamPassThrough {
  state Balances: Map<Name, int>;

  fn ensure_balance(state Map<Name, int> balances, key: Name) -> int {
    return balances.ensure(key, 0);
  }

  fn read_again(state Map<Name, int> balances, key: Name) -> int {
    return ensure_balance(balances, key);
  }

  kotoage fn run(key: Name) -> int permission(Admin) {
    return read_again(Balances, key);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(compiled.artifactBytes.length > 0);
});

test("Kotodama compiler SDK lowers scalar state parameters as durable handles", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ScalarStateParam {
  state int Counter;

  fn read(state int value) -> int {
    return value;
  }

  kotoage fn run() -> int permission(Admin) {
    Counter = 7;
    return read(Counter);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ({
      function_name: entry.function_name,
      pc_start: entry.pc_start,
      pc_end: entry.pc_end,
      bytecode_bytes: entry.bytecode_bytes,
      frame_bytes: entry.frame_bytes,
    })),
    [
      {
        function_name: "run",
        pc_start: 0,
        pc_end: 364,
        bytecode_bytes: 364,
        frame_bytes: 24,
      },
      {
        function_name: "read",
        pc_start: 364,
        pc_end: 660,
        bytecode_bytes: 296,
        frame_bytes: 32,
      },
    ],
  );
});

test("Kotodama compiler SDK mirrors scalar state parameter assignment codegen", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ScalarStateParamAssign {
  state int Counter;

  fn bump(state int value) {
    value = value + 1;
  }

  kotoage fn run() -> int permission(Admin) {
    Counter = 7;
    bump(Counter);
    return Counter;
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ({
      function_name: entry.function_name,
      pc_start: entry.pc_start,
      pc_end: entry.pc_end,
      bytecode_bytes: entry.bytecode_bytes,
      frame_bytes: entry.frame_bytes,
    })),
    [
      {
        function_name: "run",
        pc_start: 0,
        pc_end: 364,
        bytecode_bytes: 364,
        frame_bytes: 32,
      },
      {
        function_name: "bump",
        pc_start: 364,
        pc_end: 656,
        bytecode_bytes: 292,
        frame_bytes: 40,
      },
    ],
  );
  assert.equal(
    Buffer.from(readArtifactCode(compiled.artifactBytes).subarray(496, 528)).toString("hex"),
    "000a172000170a20e000006053000060000a1820000017200117172017180901",
  );
});

for (const [label, declarations] of [
  [
    "inferred",
    `let path = name("session");
    let stored = state_get(path);`,
  ],
  [
    "declared",
    `let path: Name = name("session");
    let stored: Blob = state_get(path);`,
  ],
]) {
  test(`Kotodama compiler SDK emits direct host state helper paths for ${label} Name locals`, () => {
    const compiled = compileKotodamaProgram(`
seiyaku HostStateHelpers {
  kotoage fn run() permission(Admin) {
    ${declarations}
    state_set(path, stored);
    state_del(path);
  }
}
`);

    assert.deepEqual(compiled.diagnostics, []);
    assert.deepEqual(
      compiled.budgetReport.map((entry) => ({
        function_name: entry.function_name,
        pc_start: entry.pc_start,
        pc_end: entry.pc_end,
        bytecode_bytes: entry.bytecode_bytes,
        frame_bytes: entry.frame_bytes,
      })),
      [
        {
          function_name: "run",
          pc_start: 0,
          pc_end: 356,
          bytecode_bytes: 356,
          frame_bytes: 24,
        },
      ],
    );
    const code = readArtifactCode(compiled.artifactBytes);
    assert.notEqual(code.indexOf(syscallNeedle(0x50)), -1, "state_get should stay on the direct host syscall");
    assert.notEqual(code.indexOf(syscallNeedle(0x51)), -1, "state_set should stay on the direct host syscall");
    assert.notEqual(code.indexOf(syscallNeedle(0x52)), -1, "state_del should stay on the direct host syscall");
    assert.equal(code.indexOf(syscallNeedle(0x5c)), -1, "rematerialized Name paths should not decode before state helper calls");
  });
}

test("Kotodama compiler SDK keeps scalar state handles distinct from same-block value caches", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ScalarStateParamCachedAssign {
  state int Counter;

  fn bump(state int value) {
    value = value + 1;
  }

  kotoage fn run() -> int permission(Admin) {
    let seed = 7;
    Counter = seed;
    bump(Counter);
    return Counter;
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(compiled.artifactBytes.length > 0);
});

test("Kotodama compiler SDK supports map-valued helper parameters and returns", () => {
  const compiled = compileKotodamaProgram(`
seiyaku MapReturnHelpers {
  fn make_empty() -> Map<Name, int> {
    return Map::new();
  }

  fn make_seeded() -> Map<Name, int> {
    let values: Map<Name, int> = Map::new();
    values[name("alice")] = 7;
    return values;
  }

  fn read(values: Map<Name, int>, key: Name) -> int {
    return values.get_or(key, 0);
  }

  kotoage fn run() -> int permission(Admin) {
    let empty = make_empty();
    let seeded = make_seeded();
    return read(empty, name("bob")) + read(seeded, name("alice"));
  }
}
`);
  const defaultMap = compileKotodamaProgram(`
seiyaku DefaultMap {
  kotoage fn run() -> int permission(Admin) {
    let values = Map::new();
    values[1] = 42;
    return values[1];
  }
}
`);
  const defaultMapKeyMismatch = compileKotodamaProgram(`
seiyaku DefaultMapKeyMismatch {
  fn helper() {
    let values = Map::new();
    values[name("score")] = 42;
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(compiled.codeHashHex.length, 64);
  assert.ok(compiled.artifactBytes.length > 64);
  assert.deepEqual(defaultMap.diagnostics, []);
  assert.ok(defaultMap.artifactBytes.length > 64);
  assert.equal(defaultMapKeyMismatch.artifactBytes.length, 0);
  assert.equal(
    defaultMapKeyMismatch.diagnostics[0].message,
    "semantic error: type annotation mismatch: expected int, got Name",
  );
  const code = readArtifactCode(compiled.artifactBytes);
  assert.equal(code.indexOf(syscallNeedle(0x50)), -1, "helper-returned local maps must not emit STATE_GET");
  assert.equal(code.indexOf(syscallNeedle(0x56)), -1, "helper-returned local maps must not build durable state paths");
});

test("Kotodama compiler SDK uses direct Name literal TLVs for local map keys", () => {
  const compiled = compileKotodamaProgram(`
seiyaku LocalNameMapKeys {
  kotoage fn run() -> int permission(Admin) {
    let values: Map<Name, int> = Map::new();
    values[name("alice")] = 7;
    let first = values.get_or(name("alice"), 0);
    let second = values[name("alice")];
    let seen = values.contains(name("alice"));
    return first + second + seen;
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(compiled.artifactBytes.length > 64);
  const code = readArtifactCode(compiled.artifactBytes);
  assert.equal(code.indexOf(syscallNeedle(0x5c)), -1, "local Name map literal keys should not decode before TLV comparison");
  assert.notEqual(code.indexOf(syscallNeedle(0x5f)), -1, "local Name map literal keys should compare published TLVs");
});

test("Kotodama compiler SDK supports maps inside local struct fields", () => {
  const compiled = compileKotodamaProgram(`
struct Holder { values: Map<Name, int>; }
struct Wrap { holder: Holder; }

seiyaku StructMapFields {
  fn build() -> Wrap {
    let values: Map<Name, int> = Map::new();
    values[name("alice")] = 7;
    return Wrap(Holder(values));
  }

  fn read(wrapped: Wrap) -> int {
    return wrapped.holder.values.get_or(name("alice"), 0);
  }

  kotoage fn run() -> int permission(Admin) {
    return read(build());
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(compiled.codeHashHex.length, 64);
  assert.ok(compiled.artifactBytes.length > 64);
});

test("Kotodama compiler SDK supports method calls after tuple numeric map members", () => {
  const compiled = compileKotodamaProgram(`
seiyaku TupleMapMembers {
  fn make() -> (Map<Name, int>, int) {
    let values: Map<Name, int> = Map::new();
    values[name("alice")] = 7;
    return (values, 1);
  }

  kotoage fn run() -> int permission(Admin) {
    let pair = make();
    return pair.0.get_or(name("alice"), 0) + pair.1;
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(compiled.codeHashHex.length, 64);
  assert.ok(compiled.artifactBytes.length > 64);
});

test("Kotodama compiler SDK mirrors Rust opaque aggregate helper parameter ABI", () => {
  const tupleParam = compileKotodamaProgram(`
seiyaku HelperTupleParam {
  fn sum(pair: (int, int)) -> int {
    return pair.0 + pair.1;
  }

  kotoage fn run(x: int) -> int {
    return sum((x, 2));
  }
}
`);
  const structParam = compileKotodamaProgram(`
struct Pair { a: int; b: int; }

seiyaku HelperStructParam {
  fn sum(pair: Pair) -> int {
    return pair.a + pair.b;
  }

  kotoage fn run(x: int) -> int {
    return sum(Pair(x, 2));
  }
}
`);

  for (const compiled of [tupleParam, structParam]) {
    const budgets = new Map(compiled.budgetReport.map((entry) => [entry.function_name, entry]));

    assert.deepEqual(compiled.diagnostics, []);
    assert.equal(compiled.artifactBytes.length, 1293);
    assert.deepEqual(
      { bytecode_bytes: budgets.get("sum")?.bytecode_bytes, frame_bytes: budgets.get("sum")?.frame_bytes },
      { bytecode_bytes: 92, frame_bytes: 40 },
    );
    assert.deepEqual(
      {
        bytecode_bytes: budgets.get("__entrypoint_impl__run")?.bytecode_bytes,
        frame_bytes: budgets.get("__entrypoint_impl__run")?.frame_bytes,
      },
      { bytecode_bytes: 416, frame_bytes: 48 },
    );
  }
});

test("Kotodama compiler SDK supports direct map-valued helper expressions", () => {
  const compiled = compileKotodamaProgram(`
struct Holder { values: Map<Name, int>; }

seiyaku DirectMapExpressions {
  fn make() -> Map<Name, int> {
    let values: Map<Name, int> = Map::new();
    values[name("alice")] = 7;
    return values;
  }

  fn make_pair() -> (Map<Name, int>, int) {
    return (make(), 1);
  }

  fn make_holder() -> Holder {
    return Holder(make());
  }

  fn read(values: Map<Name, int>, key: Name) -> int {
    return values.get_or(key, 0);
  }

  kotoage fn run() -> int permission(Admin) {
    let typed: Map<Name, int> = make();
    let conditional: Map<Name, int> = 1 > 0 ? make() : make();
    return make().get_or(name("alice"), 0)
      + make()[name("alice")]
      + make().contains(name("alice"))
      + make_pair().0.get_or(name("alice"), 0)
      + make_pair().0[name("alice")]
      + make_holder().values.get_or(name("alice"), 0)
      + read(make(), name("alice"))
      + (1 > 0 ? make() : make()).get_or(name("alice"), 0)
      + conditional.get_or(name("alice"), 0)
      + typed.get_or(name("alice"), 0);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(compiled.codeHashHex.length, 64);
  assert.ok(compiled.artifactBytes.length > 64);
});

test("Kotodama compiler SDK matches Rust budget for helper map-return lookups", () => {
  const compiled = compileKotodamaProgram(`
seiyaku HelperMapReturn {
  fn make() -> Map<Name, int> {
    let values: Map<Name, int> = Map::new();
    values[name("alice")] = 7;
    return values;
  }

  kotoage fn run() -> int permission(Admin) {
    return make().get_or(name("alice"), 0);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ({
      function_name: entry.function_name,
      bytecode_bytes: entry.bytecode_bytes,
      frame_bytes: entry.frame_bytes,
      pc_start: entry.pc_start,
      pc_end: entry.pc_end,
    })),
    [
      { function_name: "run", bytecode_bytes: 668, frame_bytes: 48, pc_start: 0, pc_end: 668 },
      { function_name: "make", bytecode_bytes: 228, frame_bytes: 32, pc_start: 668, pc_end: 896 },
    ],
  );
});

test("Kotodama compiler SDK rejects unsupported durable map key types", () => {
  const boolKey = compileKotodamaProgram(`
seiyaku BoolKey {
  state Values: Map<bool, int>;

  kotoage fn run() permission(Admin) {}
}
`);
  const tupleKey = compileKotodamaProgram(`
seiyaku TupleKey {
  state Values: Map<(int, int), int>;

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(boolKey.artifactBytes.length, 0);
  assert.equal(boolKey.diagnostics.length, 1);
  assert.match(boolKey.diagnostics[0].message, /state Map key type `bool` is not supported/);
  assert.equal(tupleKey.artifactBytes.length, 0);
  assert.equal(tupleKey.diagnostics.length, 1);
  assert.match(tupleKey.diagnostics[0].message, /state Map key type `\(int, int\)` is not supported/);
});

test("Kotodama compiler SDK rejects invalid assert argument shapes", () => {
  const nonBoolCondition = compileKotodamaProgram(`
seiyaku AssertCondition {
  kotoage fn run() permission(Admin) {
    assert(1);
  }
}
`);
  const boolMessage = compileKotodamaProgram(`
seiyaku AssertMessage {
  kotoage fn run() permission(Admin) {
    assert(true, false);
  }
}
`);

  assert.equal(nonBoolCondition.artifactBytes.length, 0);
  assert.equal(nonBoolCondition.diagnostics.length, 1);
  assert.match(nonBoolCondition.diagnostics[0].message, /assert expects \(bool\) or \(bool, string\|int\)/);
  assert.equal(boolMessage.artifactBytes.length, 0);
  assert.equal(boolMessage.diagnostics.length, 1);
  assert.match(boolMessage.diagnostics[0].message, /assert expects \(bool\) or \(bool, string\|int\)/);
});

test("Kotodama compiler SDK accepts Rust int-like wide numeric builtin arguments", () => {
  const compiled = compileKotodamaProgram(`
seiyaku WideNumericIntLikeBuiltins {
  kotoage fn run(amount: Amount, balance: Balance, exact: fixed_u128) permission(Admin) {
    assert(true, amount);
    require(true, exact);
    set_trigger_enabled(name("wake"), balance);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x02)), -1, "missing ABORT syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x42)), -1, "missing SET_TRIGGER_ENABLED syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x6a)), -1, "missing NUMERIC_TO_INT syscall");
});

test("Kotodama compiler SDK emits require as an aborting assertion", () => {
  const compiled = compileKotodamaProgram(`
seiyaku RequireAssertion {
  kotoage fn run() permission(Admin) {
    require(1 == 1, 7);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x02)), -1);
});

test("Kotodama compiler SDK rejects invalid require argument shapes", () => {
  const nonBoolCondition = compileKotodamaProgram(`
seiyaku RequireCondition {
  kotoage fn run() permission(Admin) {
    require(1);
  }
}
`);
  const boolMessage = compileKotodamaProgram(`
seiyaku RequireMessage {
  kotoage fn run() permission(Admin) {
    require(true, false);
  }
}
`);

  assert.equal(nonBoolCondition.artifactBytes.length, 0);
  assert.equal(nonBoolCondition.diagnostics.length, 1);
  assert.match(nonBoolCondition.diagnostics[0].message, /require expects \(bool\) or \(bool, string\|int\)/);
  assert.equal(boolMessage.artifactBytes.length, 0);
  assert.equal(boolMessage.diagnostics.length, 1);
  assert.match(boolMessage.diagnostics[0].message, /require expects \(bool\) or \(bool, string\|int\)/);
});

test("Kotodama compiler SDK parses upstream call statement sugar", () => {
  const valid = compileKotodamaProgram(`
seiyaku CallStatement {
  kotoage fn run() permission(Admin) {
    call require(1 == 1, 7);
  }
}
`);
  const invalidLiteral = compileKotodamaProgram(`
seiyaku InvalidCallStatementLiteral {
  kotoage fn run() permission(Admin) {
    call 1;
  }
}
`);
  const invalidIdentifier = compileKotodamaProgram(`
seiyaku InvalidCallStatementIdentifier {
  kotoage fn run() permission(Admin) {
    call require;
  }
}
`);
  const validHostCalls = compileKotodamaProgram(`
seiyaku CallStatementHostCalls {
  kotoage fn run() permission(Admin) {
    call transfer_asset(authority(), authority(), asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), 1);
    call set_account_detail(authority(), name("status"), json!{ value: "ok" });
  }
}
`);
  const invalidTransfer = compileKotodamaProgram(`
seiyaku InvalidCallStatementTransfer {
  kotoage fn run() permission(Admin) {
    call transfer_asset(authority(), authority(), name("rose"), 1);
  }
}
`);
  const code = readArtifactCode(valid.artifactBytes);
  const hostCallCode = readArtifactCode(validHostCalls.artifactBytes);

  assert.deepEqual(valid.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x02)), -1);
  assert.deepEqual(validHostCalls.diagnostics, []);
  assert.notEqual(hostCallCode.indexOf(syscallNeedle(0x24)), -1);
  assert.notEqual(hostCallCode.indexOf(syscallNeedle(0x1a)), -1);
  assert.equal(invalidLiteral.artifactBytes.length, 0);
  assert.equal(invalidLiteral.diagnostics.length, 1);
  assert.match(invalidLiteral.diagnostics[0].message, /call expects a function call expression/);
  assert.equal(invalidIdentifier.artifactBytes.length, 0);
  assert.equal(invalidIdentifier.diagnostics.length, 1);
  assert.match(invalidIdentifier.diagnostics[0].message, /call expects a function call expression/);
  assert.equal(invalidTransfer.artifactBytes.length, 0);
  assert.equal(invalidTransfer.diagnostics.length, 1);
  assert.match(invalidTransfer.diagnostics[0].message, /transfer_asset expects \(AccountId, AccountId, AssetDefinitionId, numeric\)/);
});

test("Kotodama compiler SDK rejects tuple equality semantically", () => {
  const compiled = compileKotodamaProgram(`
seiyaku TupleEquality {
  kotoage fn run() permission(Admin) {
    let a = (1, 2);
    let b = (1, 2);
    let same = a == b;
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(compiled.diagnostics[0].message, /equality is not supported for type \(int, int\)/);
});

test("Kotodama compiler SDK rejects unsupported in-memory map word types", () => {
  const tupleKey = compileKotodamaProgram(`
seiyaku LocalTupleKey {
  kotoage fn run() permission(Admin) {
    let values: Map<(int, int), int> = Map::new();
    let seen = values.contains((1, 2));
  }
}
`);
  const tupleValue = compileKotodamaProgram(`
seiyaku LocalTupleValue {
  kotoage fn run() permission(Admin) {
    let values: Map<int, (int, int)> = Map::new();
    let pair = values[1];
  }
}
`);

  assert.equal(tupleKey.artifactBytes.length, 0);
  assert.equal(tupleKey.diagnostics.length, 1);
  assert.match(tupleKey.diagnostics[0].message, /in-memory Map key type `\(int, int\)` is not supported/);
  assert.equal(tupleValue.artifactBytes.length, 0);
  assert.equal(tupleValue.diagnostics.length, 1);
  assert.match(tupleValue.diagnostics[0].message, /in-memory Map value type `\(int, int\)` is not supported/);
});

test("Kotodama compiler SDK enforces Rust on-chain map key policy", () => {
  const localBytesMap = compileKotodamaProgram(`
seiyaku LocalBytesMapPolicy {
  kotoage fn run() permission(Admin) {
    let values: Map<bytes, bytes> = Map::new();
    values[norito_bytes("0x0102")] = norito_bytes("0x0304");
    for (key, value) in values #[bounded(1)] {
      info(tlv_len(key) + tlv_len(value));
    }
  }
}
`);
  const localStringMap = compileKotodamaProgram(`
seiyaku LocalStringMapPolicy {
  kotoage fn run() permission(Admin) {
    let values: Map<String, int> = Map::new();
    values["alpha"] = 4;
  }
}
`);
  const returnBytesMap = compileKotodamaProgram(`
seiyaku ReturnBytesMapPolicy {
  fn helper() -> Map<bytes, int> {
    return Map::new();
  }

  kotoage fn run() -> int permission(Admin) {
    return 1;
  }
}
`);

  assert.equal(localBytesMap.artifactBytes.length, 0);
  assert.equal(localBytesMap.diagnostics.length, 1);
  assert.equal(
    localBytesMap.diagnostics[0].message,
    "on-chain profile forbids map with key type `bytes` in binding `values` in `run`. Supported key types: int, AccountId, AssetDefinitionId, AssetId, NftId, DomainId, Name, DataSpaceId, AxtDescriptor, AssetHandle, ProofBlob.\n"
      + "on-chain profile forbids map with key type `bytes` in map assignment in `run`. Supported key types: int, AccountId, AssetDefinitionId, AssetId, NftId, DomainId, Name, DataSpaceId, AxtDescriptor, AssetHandle, ProofBlob.\n"
      + "on-chain profile forbids map with key type `bytes` in map iteration in `run`. Supported key types: int, AccountId, AssetDefinitionId, AssetId, NftId, DomainId, Name, DataSpaceId, AxtDescriptor, AssetHandle, ProofBlob.",
  );
  assert.equal(localStringMap.artifactBytes.length, 0);
  assert.equal(localStringMap.diagnostics.length, 1);
  assert.match(
    localStringMap.diagnostics[0].message,
    /on-chain profile forbids map with key type `string` in binding `values` in `run`/,
  );
  assert.equal(returnBytesMap.artifactBytes.length, 0);
  assert.equal(returnBytesMap.diagnostics.length, 1);
  assert.equal(
    returnBytesMap.diagnostics[0].message,
    "on-chain profile forbids map with key type `bytes` in return in `helper`. Supported key types: int, AccountId, AssetDefinitionId, AssetId, NftId, DomainId, Name, DataSpaceId, AxtDescriptor, AssetHandle, ProofBlob.\n"
      + "on-chain profile forbids map with key type `bytes` in function `helper` return type. Supported key types: int, AccountId, AssetDefinitionId, AssetId, NftId, DomainId, Name, DataSpaceId, AxtDescriptor, AssetHandle, ProofBlob.",
  );
});

test("Kotodama compiler SDK rejects loop control outside loops semantically", () => {
  const breakOutside = compileKotodamaProgram(`
seiyaku BreakOutside {
  kotoage fn run() permission(Admin) {
    break;
  }
}
`);
  const continueOutside = compileKotodamaProgram(`
seiyaku ContinueOutside {
  kotoage fn run() permission(Admin) {
    continue;
  }
}
`);

  assert.equal(breakOutside.artifactBytes.length, 0);
  assert.equal(breakOutside.diagnostics.length, 1);
  assert.match(breakOutside.diagnostics[0].message, /E_BREAK_OUTSIDE_LOOP/);
  assert.equal(continueOutside.artifactBytes.length, 0);
  assert.equal(continueOutside.diagnostics.length, 1);
  assert.match(continueOutside.diagnostics[0].message, /E_CONTINUE_OUTSIDE_LOOP/);
});

test("Kotodama compiler SDK rejects invalid numeric and logical operands semantically", () => {
  const accountArithmetic = compileKotodamaProgram(`
seiyaku AccountArithmetic {
  fn helper(who: AccountId) {
    let next = who + 1;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const notInt = compileKotodamaProgram(`
seiyaku NotInt {
  fn helper() {
    let ok = !1;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const logicalInt = compileKotodamaProgram(`
seiyaku LogicalInt {
  fn helper() {
    let ok = 1 && 2;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const boolArithmetic = compileKotodamaProgram(`
seiyaku BoolArithmetic {
  fn helper(flag: bool) {
    let next = flag + 1;
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(accountArithmetic.artifactBytes.length, 0);
  assert.equal(accountArithmetic.diagnostics.length, 1);
  assert.match(accountArithmetic.diagnostics[0].message, /Add expects int operands/);
  assert.equal(notInt.artifactBytes.length, 0);
  assert.equal(notInt.diagnostics.length, 1);
  assert.match(notInt.diagnostics[0].message, /unary '!' expects bool/);
  assert.equal(logicalInt.artifactBytes.length, 0);
  assert.equal(logicalInt.diagnostics.length, 1);
  assert.match(logicalInt.diagnostics[0].message, /And expects bool operands/);
  assert.deepEqual(boolArithmetic.diagnostics, []);
});

test("Kotodama compiler SDK rejects invalid condition types semantically", () => {
  const ifInt = compileKotodamaProgram(`
seiyaku IfInt {
  fn helper() {
    if 1 {
      let x = 1;
    }
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const whileInt = compileKotodamaProgram(`
seiyaku WhileInt {
  fn helper() {
    while 1 {
      break;
    }
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const conditionalInt = compileKotodamaProgram(`
seiyaku ConditionalInt {
  fn helper() {
    let value = 1 ? 2 : 3;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const conditionalBranchMismatch = compileKotodamaProgram(`
seiyaku ConditionalBranchMismatch {
  fn helper(flag: bool) {
    let value = flag ? 2 : name!("key");
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(ifInt.artifactBytes.length, 0);
  assert.equal(ifInt.diagnostics.length, 1);
  assert.match(ifInt.diagnostics[0].message, /if condition must be bool/);
  assert.equal(whileInt.artifactBytes.length, 0);
  assert.equal(whileInt.diagnostics.length, 1);
  assert.match(whileInt.diagnostics[0].message, /while condition must be bool/);
  assert.equal(conditionalInt.artifactBytes.length, 0);
  assert.equal(conditionalInt.diagnostics.length, 1);
  assert.match(conditionalInt.diagnostics[0].message, /conditional expects a bool condition/);
  assert.equal(conditionalBranchMismatch.artifactBytes.length, 0);
  assert.equal(conditionalBranchMismatch.diagnostics.length, 1);
  assert.match(conditionalBranchMismatch.diagnostics[0].message, /conditional branches must have the same type/);
});

test("Kotodama compiler SDK rejects invalid loop count types semantically", () => {
  const invalidRangeCount = compileKotodamaProgram(`
seiyaku InvalidRangeCount {
  fn helper() {
    for i in range(name("limit")) {
      let value = i;
    }
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidRepeatCount = compileKotodamaProgram(`
seiyaku InvalidRepeatCount {
  fn helper() {
    for (let i = 0; i < name("limit"); i = i + 1) {
      let value = i;
    }
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const validLoopCounts = compileKotodamaProgram(`
seiyaku ValidLoopCounts {
  fn helper(limit: int) {
    for i in range(limit) {
      let value = i;
    }
    for (let j = 0; j < limit; j = j + 1) {
      let value = limit;
    }
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(invalidRangeCount.artifactBytes.length, 0);
  assert.equal(invalidRangeCount.diagnostics.length, 1);
  assert.match(invalidRangeCount.diagnostics[0].message, /for range count must be int/);
  assert.equal(invalidRepeatCount.artifactBytes.length, 0);
  assert.equal(invalidRepeatCount.diagnostics.length, 1);
  assert.match(invalidRepeatCount.diagnostics[0].message, /repeat count must be int/);
  assert.deepEqual(validLoopCounts.diagnostics, []);
});

test("Kotodama compiler SDK accepts Rust-style for loops", () => {
  const sumLoop = compileKotodamaProgram(`
seiyaku RustStyleForSum {
  fn sum(limit: int) -> int {
    let total = 0;
    for let i = 0; i < limit; i = i + 1 {
      total = total + i;
    }
    return total;
  }

  kotoage fn run() -> int permission(Admin) {
    return sum(4);
  }
}
`);
  const initBindingEscapes = compileKotodamaProgram(`
seiyaku RustStyleForInitScope {
  kotoage fn run() -> int permission(Admin) {
    for let i = 0; i < 1; i = i + 1 {
      let body_value = i;
    }
    return i;
  }
}
`);

  assert.deepEqual(sumLoop.diagnostics, []);
  assert.ok(sumLoop.artifactBytes.length > 64);
  assert.deepEqual(initBindingEscapes.diagnostics, []);
  assert.ok(initBindingEscapes.artifactBytes.length > 64);
});

test("Kotodama compiler SDK mirrors Rust-style for loop semantic scoping", () => {
  const stepBindingIsNotInBody = compileKotodamaProgram(`
seiyaku RustStyleForStepScope {
  fn helper() -> int {
    for let i = 0; i < 1; let step_value = 1 {
      let body_value = step_value;
    }
    return 0;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const bodyBindingDoesNotEscape = compileKotodamaProgram(`
seiyaku RustStyleForBodyScope {
  fn helper() -> int {
    for let i = 0; i < 1; i = i + 1 {
      let body_value = i;
    }
    return body_value;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const aggregateInitializer = compileKotodamaProgram(`
seiyaku RustStyleForAggregateInit {
  fn helper() {
    for let pair = (1, 2); pair.0 < 3; pair = (pair.0 + 1, pair.1) {
    }
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const aggregateStep = compileKotodamaProgram(`
seiyaku RustStyleForAggregateStep {
  fn helper() {
    for let i = 0; i < 1; let pair = (1, 2) {
    }
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const nonBoolCondition = compileKotodamaProgram(`
seiyaku RustStyleForCondition {
  fn helper() {
    for let i = 0; 1; i = i + 1 {
    }
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(stepBindingIsNotInBody.artifactBytes.length, 0);
  assert.equal(stepBindingIsNotInBody.diagnostics.length, 1);
  assert.match(stepBindingIsNotInBody.diagnostics[0].message, /undefined variable step_value/);
  assert.equal(bodyBindingDoesNotEscape.artifactBytes.length, 0);
  assert.equal(bodyBindingDoesNotEscape.diagnostics.length, 1);
  assert.match(bodyBindingDoesNotEscape.diagnostics[0].message, /undefined variable body_value/);
  assert.equal(aggregateInitializer.artifactBytes.length, 0);
  assert.equal(aggregateInitializer.diagnostics.length, 1);
  assert.match(aggregateInitializer.diagnostics[0].message, /E0005: for-loop initializer must be a simple let or expression/);
  assert.equal(aggregateStep.artifactBytes.length, 0);
  assert.equal(aggregateStep.diagnostics.length, 1);
  assert.match(aggregateStep.diagnostics[0].message, /E0006: for-loop step must be a simple let or expression/);
  assert.equal(nonBoolCondition.artifactBytes.length, 0);
  assert.equal(nonBoolCondition.diagnostics.length, 1);
  assert.match(nonBoolCondition.diagnostics[0].message, /for condition must be bool/);
});

test("Kotodama compiler SDK rejects invalid user function arguments semantically", () => {
  const accountToInt = compileKotodamaProgram(`
seiyaku AccountToInt {
  fn helper(value: int) -> int {
    return value;
  }

  fn caller(who: AccountId) {
    let next = helper(who);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const intToAccount = compileKotodamaProgram(`
seiyaku IntToAccount {
  fn helper(who: AccountId) -> int {
    return 1;
  }

  fn caller() {
    let next = helper(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const boolToInt = compileKotodamaProgram(`
seiyaku BoolToInt {
  fn helper(value: int) -> int {
    return value;
  }

  fn caller(flag: bool) {
    let next = helper(flag);
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(accountToInt.artifactBytes.length, 0);
  assert.equal(accountToInt.diagnostics.length, 1);
  assert.match(accountToInt.diagnostics[0].message, /type annotation mismatch: expected int, got AccountId/);
  assert.equal(intToAccount.artifactBytes.length, 0);
  assert.equal(intToAccount.diagnostics.length, 1);
  assert.match(intToAccount.diagnostics[0].message, /type annotation mismatch: expected AccountId, got int/);
  assert.deepEqual(boolToInt.diagnostics, []);
});

test("Kotodama compiler SDK rejects invalid constructor arguments semantically", () => {
  const structMismatch = compileKotodamaProgram(`
seiyaku StructMismatch {
  struct Pair {
    count: int,
    owner: AccountId,
  }

  fn helper() {
    let pair = Pair(1, 2);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const pointerMismatch = compileKotodamaProgram(`
seiyaku PointerMismatch {
  fn helper(who: AccountId) {
    let key = name(who);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const stringBinding = compileKotodamaProgram(`
seiyaku StringBinding {
  fn helper() {
    let raw = "pool";
    let key = name(raw);
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(structMismatch.artifactBytes.length, 0);
  assert.equal(structMismatch.diagnostics.length, 1);
  assert.match(structMismatch.diagnostics[0].message, /type annotation mismatch: expected AccountId, got int/);
  assert.equal(pointerMismatch.artifactBytes.length, 0);
  assert.equal(pointerMismatch.diagnostics.length, 1);
  assert.match(pointerMismatch.diagnostics[0].message, /name expects string, Name, or Blob/);
  assert.deepEqual(stringBinding.diagnostics, []);
});

test("Kotodama compiler SDK rejects invalid local assignment types semantically", () => {
  const declaredMismatch = compileKotodamaProgram(`
seiyaku DeclaredMismatch {
  fn helper() {
    let owner: AccountId = 1;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const reassignMismatch = compileKotodamaProgram(`
seiyaku ReassignMismatch {
  fn helper(owner: AccountId) {
    let next = owner;
    next = 1;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const mapValueMismatch = compileKotodamaProgram(`
seiyaku MapValueMismatch {
  fn helper() {
    let owners: Map<Name, AccountId> = Map::new();
    owners[name!("alice")] = 1;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const boolAssignment = compileKotodamaProgram(`
seiyaku BoolAssignment {
  fn helper(flag: bool) {
    let value: int = flag;
    value = false;
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(declaredMismatch.artifactBytes.length, 0);
  assert.equal(declaredMismatch.diagnostics.length, 1);
  assert.match(declaredMismatch.diagnostics[0].message, /type annotation mismatch: expected AccountId, got int/);
  assert.equal(reassignMismatch.artifactBytes.length, 0);
  assert.equal(reassignMismatch.diagnostics.length, 1);
  assert.match(reassignMismatch.diagnostics[0].message, /type annotation mismatch: expected AccountId, got int/);
  assert.equal(mapValueMismatch.artifactBytes.length, 0);
  assert.equal(mapValueMismatch.diagnostics.length, 1);
  assert.match(mapValueMismatch.diagnostics[0].message, /type annotation mismatch: expected AccountId, got int/);
  assert.deepEqual(boolAssignment.diagnostics, []);
});

test("Kotodama compiler SDK rejects invalid return types semantically", () => {
  const returnMismatch = compileKotodamaProgram(`
seiyaku ReturnMismatch {
  fn helper() -> AccountId {
    return 1;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const missingValue = compileKotodamaProgram(`
seiyaku MissingReturnValue {
  fn helper() -> int {
    return;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const boolReturn = compileKotodamaProgram(`
seiyaku BoolReturn {
  fn helper(flag: bool) -> int {
    return flag;
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(returnMismatch.artifactBytes.length, 0);
  assert.equal(returnMismatch.diagnostics.length, 1);
  assert.match(returnMismatch.diagnostics[0].message, /return type mismatch: type annotation mismatch: expected AccountId, got int/);
  assert.equal(missingValue.artifactBytes.length, 0);
  assert.equal(missingValue.diagnostics.length, 1);
  assert.match(missingValue.diagnostics[0].message, /return type mismatch: expected value/);
  assert.deepEqual(boolReturn.diagnostics, []);
});

test("Kotodama compiler SDK accepts upstream unit return type spellings", () => {
  const namedUnit = compileKotodamaProgram(`
seiyaku NamedUnitReturn {
  fn helper() -> unit {
    return;
  }

  kotoage fn run() permission(Admin) {
    helper();
  }
}
`);
  const tupleUnit = compileKotodamaProgram(`
seiyaku TupleUnitReturn {
  fn helper() -> () {
    return;
  }

  kotoage fn run() permission(Admin) {
    helper();
  }
}
`);
  const invalidNamedValue = compileKotodamaProgram(`
seiyaku NamedUnitReturnValue {
  fn helper() -> unit {
    return 1;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidTupleValue = compileKotodamaProgram(`
seiyaku TupleUnitReturnValue {
  fn helper() -> () {
    return 1;
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.deepEqual(namedUnit.diagnostics, []);
  assert.deepEqual(tupleUnit.diagnostics, []);
  assert.equal(invalidNamedValue.artifactBytes.length, 0);
  assert.equal(invalidNamedValue.diagnostics.length, 1);
  assert.match(
    invalidNamedValue.diagnostics[0].message,
    /return type mismatch: unexpected value/,
  );
  assert.equal(invalidTupleValue.artifactBytes.length, 0);
  assert.equal(invalidTupleValue.diagnostics.length, 1);
  assert.match(
    invalidTupleValue.diagnostics[0].message,
    /return type mismatch: type annotation mismatch: expected \(\), got int/,
  );
});

test("Kotodama compiler SDK rejects undefined variables semantically", () => {
  const missingValue = compileKotodamaProgram(`
seiyaku MissingValue {
  fn helper() {
    let value = missing + 1;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const missingReturn = compileKotodamaProgram(`
seiyaku MissingReturnValue {
  fn helper() -> int {
    return missing;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const missingAssignmentTarget = compileKotodamaProgram(`
seiyaku MissingAssignmentTarget {
  fn helper() {
    missing = 1;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const constValue = compileKotodamaProgram(`
seiyaku ConstValue {
  const ONE: int = 1;
  const TWO = ONE;

  fn helper() {
    let value: int = TWO + 1;
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(missingValue.artifactBytes.length, 0);
  assert.equal(missingValue.diagnostics.length, 1);
  assert.match(missingValue.diagnostics[0].message, /undefined variable missing/);
  assert.equal(missingReturn.artifactBytes.length, 0);
  assert.equal(missingReturn.diagnostics.length, 1);
  assert.match(missingReturn.diagnostics[0].message, /undefined variable missing/);
  assert.equal(missingAssignmentTarget.artifactBytes.length, 0);
  assert.equal(missingAssignmentTarget.diagnostics.length, 1);
  assert.match(missingAssignmentTarget.diagnostics[0].message, /undefined variable missing/);
  assert.deepEqual(constValue.diagnostics, []);
});

test("Kotodama compiler SDK rejects invalid const initializers semantically", () => {
  const callInitializer = compileKotodamaProgram(`
seiyaku ConstCall {
  const ONE = isqrt(4);

  fn helper() {
    let value = ONE;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const binaryInitializer = compileKotodamaProgram(`
seiyaku ConstBinary {
  const ONE = 1 + 1;

  fn helper() {
    let value = ONE;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const forwardReference = compileKotodamaProgram(`
seiyaku ConstForwardReference {
  const TWO = ONE;
  const ONE = 1;

  fn helper() {
    let value = TWO;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidNegation = compileKotodamaProgram(`
seiyaku ConstNegation {
  const BAD = -true;

  fn helper() {
    let value = BAD;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const validPreviousConst = compileKotodamaProgram(`
seiyaku ConstPreviousValue {
  const ONE = 1;
  const TWO: int = ONE;
  const RAW: String = "pool";

  fn helper() {
    let value: int = TWO;
    let key = name(RAW);
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(callInitializer.artifactBytes.length, 0);
  assert.equal(callInitializer.diagnostics.length, 1);
  assert.match(callInitializer.diagnostics[0].message, /const initializers must be literal values or previously declared constants/);
  assert.equal(binaryInitializer.artifactBytes.length, 0);
  assert.equal(binaryInitializer.diagnostics.length, 1);
  assert.match(binaryInitializer.diagnostics[0].message, /const initializers must be literal values or previously declared constants/);
  assert.equal(forwardReference.artifactBytes.length, 0);
  assert.equal(forwardReference.diagnostics.length, 1);
  assert.match(forwardReference.diagnostics[0].message, /const `ONE` is undefined or not yet declared/);
  assert.equal(invalidNegation.artifactBytes.length, 0);
  assert.equal(invalidNegation.diagnostics.length, 1);
  assert.match(invalidNegation.diagnostics[0].message, /const unary '-' expects an integer literal or integer const/);
  assert.deepEqual(validPreviousConst.diagnostics, []);
});

test("Kotodama compiler SDK rejects invalid member and index expressions semantically", () => {
  const tupleField = compileKotodamaProgram(`
seiyaku TupleField {
  fn helper() {
    let pair = (1, 2);
    let value = pair.foo;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const tupleOutOfBounds = compileKotodamaProgram(`
seiyaku TupleOutOfBounds {
  fn helper() {
    let pair = (1, 2);
    let value = pair.2;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const structField = compileKotodamaProgram(`
seiyaku StructField {
  struct Pair {
    a: int,
  }

  fn helper() {
    let pair = Pair(1);
    let value = pair.b;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const nonMapIndex = compileKotodamaProgram(`
seiyaku NonMapIndex {
  fn helper() {
    let value = 1;
    let item = value[name!("key")];
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const mapKeyMismatch = compileKotodamaProgram(`
seiyaku MapKeyMismatch {
  fn helper() {
    let values: Map<Name, int> = Map::new();
    let item = values[1];
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(tupleField.artifactBytes.length, 0);
  assert.equal(tupleField.diagnostics.length, 1);
  assert.match(tupleField.diagnostics[0].message, /unknown field 'foo' on tuple/);
  assert.equal(tupleOutOfBounds.artifactBytes.length, 0);
  assert.equal(tupleOutOfBounds.diagnostics.length, 1);
  assert.match(tupleOutOfBounds.diagnostics[0].message, /tuple index 2 out of bounds \(len=2\)/);
  assert.equal(structField.artifactBytes.length, 0);
  assert.equal(structField.diagnostics.length, 1);
  assert.match(structField.diagnostics[0].message, /unknown field 'b' on struct Pair \(available: a\)/);
  assert.equal(nonMapIndex.artifactBytes.length, 0);
  assert.equal(nonMapIndex.diagnostics.length, 1);
  assert.match(nonMapIndex.diagnostics[0].message, /indexing not supported on this type/);
  assert.equal(mapKeyMismatch.artifactBytes.length, 0);
  assert.equal(mapKeyMismatch.diagnostics.length, 1);
  assert.match(mapKeyMismatch.diagnostics[0].message, /type annotation mismatch: expected Name, got int/);
});

test("Kotodama compiler SDK rejects invalid builtin call arguments semantically", () => {
  const invalidIsqrt = compileKotodamaProgram(`
seiyaku InvalidIsqrt {
  fn helper() {
    let value = isqrt("x");
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidContainsTarget = compileKotodamaProgram(`
seiyaku InvalidContainsTarget {
  fn helper() {
    let not_map = 1;
    let seen = not_map.contains(name!("key"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidMapKey = compileKotodamaProgram(`
seiyaku InvalidMapKey {
  fn helper() {
    let values: Map<Name, int> = Map::new();
    let item = values.get_or(1, 0);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const missingPointerDefault = compileKotodamaProgram(`
seiyaku MissingPointerDefault {
  fn helper() {
    let values: Map<Name, AccountId> = Map::new();
    let item = values.get_or(name!("owner"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidGetOrDefaultArity = compileKotodamaProgram(`
seiyaku InvalidGetOrDefaultArity {
  fn helper() {
    let values: Map<Name, int> = Map::new();
    let item = get_or_default(values, name("score"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidGetOrDefaultValue = compileKotodamaProgram(`
seiyaku InvalidGetOrDefaultValue {
  fn helper() {
    let values: Map<Name, int> = Map::new();
    let item = get_or_default(values, name("score"), name("bad"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidJsonGetter = compileKotodamaProgram(`
seiyaku InvalidJsonGetter {
  fn helper() {
    let not_json = 1;
    let item = not_json.get_int(name("amount"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidJsonObjectArity = compileKotodamaProgram(`
seiyaku InvalidJsonObjectArity {
  fn helper() {
    let payload = json_object(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidJsonSetIntKey = compileKotodamaProgram(`
seiyaku InvalidJsonSetIntKey {
  fn helper() {
    let payload = json_object();
    let next = json_set_int(payload, 1, 7);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidJsonSetIntValue = compileKotodamaProgram(`
seiyaku InvalidJsonSetIntValue {
  fn helper() {
    let payload = json_object();
    let next = json_set_int(payload, name("bad"), json("{}"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidJsonSetAccountIdValue = compileKotodamaProgram(`
seiyaku InvalidJsonSetAccountIdValue {
  fn helper() {
    let payload = json_object();
    let next = json_set_account_id(payload, name("owner"), 1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidPathBase = compileKotodamaProgram(`
seiyaku InvalidPathBase {
  fn helper() {
    let payload = json_object();
    let item = payload.path(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidPathKey = compileKotodamaProgram(`
seiyaku InvalidPathKey {
  fn helper() {
    let item = name("base").path(name("segment"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidKeysTakeMap = compileKotodamaProgram(`
seiyaku InvalidKeysTakeMap {
  fn helper() {
    let values: Map<Name, int> = Map::new();
    let item = keys_take2(values, 0, 0);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidKeysValuesWhich = compileKotodamaProgram(`
seiyaku InvalidKeysValuesWhich {
  fn helper() {
    let values: Map<int, int> = Map::new();
    let item = keys_values_take2(values, 0, name("which"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidJsonGetterMethod = compileKotodamaProgram(`
seiyaku InvalidJsonGetterMethod {
  fn helper() {
    let payload = json!{ amount: 7 };
    let item = payload.get_numeric(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidBlobHexGetterMethod = compileKotodamaProgram(`
seiyaku InvalidBlobHexGetterMethod {
  fn helper() {
    let payload = json!{ proof: "010203" };
    let item = payload.get_blob_hex(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidBlockHeightArity = compileKotodamaProgram(`
seiyaku InvalidBlockHeightArity {
  fn helper() {
    let height = block_height(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidTriggerEventArity = compileKotodamaProgram(`
seiyaku InvalidTriggerEventArity {
  fn helper() {
    let item = trigger_event(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidCreateNftsArity = compileKotodamaProgram(`
seiyaku InvalidCreateNftsArity {
  kotoage fn run() {
    create_nfts_for_all_users(1);
  }
}
`);
  const invalidAssertEqBool = compileKotodamaProgram(`
seiyaku InvalidAssertEqBool {
  fn helper() {
    assert_eq(true, 1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidAssertEqPointer = compileKotodamaProgram(`
seiyaku InvalidAssertEqPointer {
  fn helper() {
    let payload = norito_bytes("0x0102");
    assert_eq(payload, payload);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidPubkgen = compileKotodamaProgram(`
seiyaku InvalidPubkgen {
  fn helper() {
    let value = pubkgen(name("seed"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidValcom = compileKotodamaProgram(`
seiyaku InvalidValcom {
  fn helper() {
    let value = valcom(1, name("blind"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const validBuiltins = compileKotodamaProgram(`
seiyaku ValidBuiltins {
  fn helper() {
    let values: Map<Name, int> = Map::new();
    let key = name("score");
    let item = values.get_or(key, 0);
    let defaulted = get_or_default(values, key, 9);
    let seen = values.contains(key);
    let pairs: Map<int, int> = Map::new();
    pairs[11] = 22;
    let first_key = keys_take2(pairs, 0, 0);
    let first_value = std::map::values_take2(pairs, 0, 0);
    let first_pair = keys_values_take2(pairs, 0, 0);
    let payload = json!{ amount: 7, "meta": "ok", kind: "mint", owner: "alice@wonderland", asset: "rose#wonderland", nft: "n0$wonderland.universal", proof: "010203" };
    let amount = payload.get_int(name("amount"));
    let numeric = payload.get_numeric(name("amount"));
    let meta = payload.get_json(name("meta"));
    let kind = payload.get_name(name("kind"));
    let owner = payload.get_account_id(name("owner"));
    let asset = payload.get_asset_definition_id(name("asset"));
    let nft = payload.get_nft_id(name("nft"));
    let proof = payload.get_blob_hex(name("proof"));
    let built = json_set_account_id(json_set_int(json_object(), key, item), name("owner"), owner);
    info(amount);
    let root = isqrt(16);
    let height = block_height();
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(invalidIsqrt.artifactBytes.length, 0);
  assert.equal(invalidIsqrt.diagnostics.length, 1);
  assert.match(invalidIsqrt.diagnostics[0].message, /isqrt expects \(int\)/);
  assert.equal(invalidContainsTarget.artifactBytes.length, 0);
  assert.equal(invalidContainsTarget.diagnostics.length, 1);
  assert.match(invalidContainsTarget.diagnostics[0].message, /contains expects Map<K,V> as first arg, got int/);
  assert.equal(invalidMapKey.artifactBytes.length, 0);
  assert.equal(invalidMapKey.diagnostics.length, 1);
  assert.match(invalidMapKey.diagnostics[0].message, /type annotation mismatch: expected Name, got int/);
  assert.equal(missingPointerDefault.artifactBytes.length, 0);
  assert.equal(missingPointerDefault.diagnostics.length, 1);
  assert.match(missingPointerDefault.diagnostics[0].message, /get_or requires an explicit default for pointer-valued maps/);
  assert.equal(invalidGetOrDefaultArity.artifactBytes.length, 0);
  assert.equal(invalidGetOrDefaultArity.diagnostics.length, 1);
  assert.match(invalidGetOrDefaultArity.diagnostics[0].message, /get_or_default expects \(Map<K,V>, K, V\)/);
  assert.equal(invalidGetOrDefaultValue.artifactBytes.length, 0);
  assert.equal(invalidGetOrDefaultValue.diagnostics.length, 1);
  assert.match(invalidGetOrDefaultValue.diagnostics[0].message, /type annotation mismatch: expected int, got Name/);
  assert.equal(invalidJsonGetter.artifactBytes.length, 0);
  assert.equal(invalidJsonGetter.diagnostics.length, 1);
  assert.match(invalidJsonGetter.diagnostics[0].message, /get_int expects \(Json, Name\)/);
  assert.equal(invalidJsonObjectArity.artifactBytes.length, 0);
  assert.equal(invalidJsonObjectArity.diagnostics.length, 1);
  assert.match(invalidJsonObjectArity.diagnostics[0].message, /json_object expects no arguments/);
  assert.equal(invalidJsonSetIntKey.artifactBytes.length, 0);
  assert.equal(invalidJsonSetIntKey.diagnostics.length, 1);
  assert.match(invalidJsonSetIntKey.diagnostics[0].message, /json_set_int expects \(Json, Name, int\)/);
  assert.equal(invalidJsonSetIntValue.artifactBytes.length, 0);
  assert.equal(invalidJsonSetIntValue.diagnostics.length, 1);
  assert.match(invalidJsonSetIntValue.diagnostics[0].message, /json_set_int expects \(Json, Name, int\)/);
  assert.equal(invalidJsonSetAccountIdValue.artifactBytes.length, 0);
  assert.equal(invalidJsonSetAccountIdValue.diagnostics.length, 1);
  assert.match(invalidJsonSetAccountIdValue.diagnostics[0].message, /json_set_account_id expects \(Json, Name, AccountId\)/);
  assert.equal(invalidPathBase.artifactBytes.length, 0);
  assert.equal(invalidPathBase.diagnostics.length, 1);
  assert.match(invalidPathBase.diagnostics[0].message, /path expects \(Name, int\|Blob\|bytes\)/);
  assert.equal(invalidPathKey.artifactBytes.length, 0);
  assert.equal(invalidPathKey.diagnostics.length, 1);
  assert.match(invalidPathKey.diagnostics[0].message, /path expects \(Name, int\|Blob\|bytes\)/);
  assert.equal(invalidKeysTakeMap.artifactBytes.length, 0);
  assert.equal(invalidKeysTakeMap.diagnostics.length, 1);
  assert.match(invalidKeysTakeMap.diagnostics[0].message, /keys_take2 expects Map<int,int> as first arg, got map<Name, int>/);
  assert.equal(invalidKeysValuesWhich.artifactBytes.length, 0);
  assert.equal(invalidKeysValuesWhich.diagnostics.length, 1);
  assert.match(invalidKeysValuesWhich.diagnostics[0].message, /keys_values_take2 expects \(Map<int,int>, int, int\)/);
  assert.equal(invalidJsonGetterMethod.artifactBytes.length, 0);
  assert.equal(invalidJsonGetterMethod.diagnostics.length, 1);
  assert.match(invalidJsonGetterMethod.diagnostics[0].message, /get_numeric expects \(Json, Name\)/);
  assert.equal(invalidBlobHexGetterMethod.artifactBytes.length, 0);
  assert.equal(invalidBlobHexGetterMethod.diagnostics.length, 1);
  assert.match(invalidBlobHexGetterMethod.diagnostics[0].message, /get_blob_hex expects \(Json, Name\)/);
  assert.equal(invalidBlockHeightArity.artifactBytes.length, 0);
  assert.equal(invalidBlockHeightArity.diagnostics.length, 1);
  assert.match(invalidBlockHeightArity.diagnostics[0].message, /block_height expects no arguments/);
  assert.equal(invalidTriggerEventArity.artifactBytes.length, 0);
  assert.equal(invalidTriggerEventArity.diagnostics.length, 1);
  assert.match(invalidTriggerEventArity.diagnostics[0].message, /trigger_event expects no arguments/);
  assert.equal(invalidCreateNftsArity.artifactBytes.length, 0);
  assert.equal(invalidCreateNftsArity.diagnostics.length, 1);
  assert.match(invalidCreateNftsArity.diagnostics[0].message, /create_nfts_for_all_users expects no arguments/);
  assert.equal(invalidAssertEqBool.artifactBytes.length, 0);
  assert.equal(invalidAssertEqBool.diagnostics.length, 1);
  assert.match(invalidAssertEqBool.diagnostics[0].message, /assert_eq expects two int args/);
  assert.equal(invalidAssertEqPointer.artifactBytes.length, 0);
  assert.equal(invalidAssertEqPointer.diagnostics.length, 1);
  assert.match(invalidAssertEqPointer.diagnostics[0].message, /assert_eq expects two int args/);
  assert.equal(invalidPubkgen.artifactBytes.length, 0);
  assert.equal(invalidPubkgen.diagnostics.length, 1);
  assert.match(invalidPubkgen.diagnostics[0].message, /pubkgen expects one int arg/);
  assert.equal(invalidValcom.artifactBytes.length, 0);
  assert.equal(invalidValcom.diagnostics.length, 1);
  assert.match(invalidValcom.diagnostics[0].message, /valcom expects two int args/);
  assert.deepEqual(validBuiltins.diagnostics, []);
});

test("Kotodama compiler SDK rejects upstream-removed helper spellings", () => {
  const removedFreeMap = compileKotodamaProgram(`
seiyaku RemovedFreeMap {
  fn helper() {
    let values: Map<Name, int> = Map::new();
    let key = name("score");
    let seen = contains(values, key);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const removedMethodMap = compileKotodamaProgram(`
seiyaku RemovedMethodMap {
  fn helper() {
    let values: Map<Name, int> = Map::new();
    let key = name("score");
    let inserted = values.get_or_insert_default(key, 7);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const removedFreeJson = compileKotodamaProgram(`
seiyaku RemovedFreeJson {
  fn helper() {
    let payload = json!{ amount: 7 };
    let amount = json_get_int(payload, name("amount"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const removedMethodJson = compileKotodamaProgram(`
seiyaku RemovedMethodJson {
  fn helper() {
    let payload = json!{ amount: 7 };
    let amount = payload.json_get_int(name("amount"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const removedStdMap = compileKotodamaProgram(`
seiyaku RemovedStdMap {
  fn helper() {
    let values: Map<int, int> = Map::new();
    let seen = std::map::contains(values, 1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const removedHostPath = compileKotodamaProgram(`
seiyaku RemovedHostPath {
  fn helper() -> Name {
    return host::path(name("x"), 1);
  }
}
`);

  assert.equal(removedFreeMap.artifactBytes.length, 0);
  assert.equal(removedFreeMap.diagnostics.length, 1);
  assert.equal(removedFreeMap.diagnostics[0].message, 'parser error: {error}: expected `contains(...)` was removed; use `map.contains(key)` but found Ident("contains")');
  assert.equal(removedMethodMap.artifactBytes.length, 0);
  assert.equal(removedMethodMap.diagnostics.length, 1);
  assert.equal(removedMethodMap.diagnostics[0].message, 'parser error: {error}: expected `map.get_or_insert_default(key, default)` was removed; use `map.ensure(key, default)` but found Ident("get_or_insert_default")');
  assert.equal(removedFreeJson.artifactBytes.length, 0);
  assert.equal(removedFreeJson.diagnostics.length, 1);
  assert.equal(removedFreeJson.diagnostics[0].message, 'parser error: {error}: expected `get_int(...)` was removed as a free helper; use `json.get_int(key)` but found Ident("json_get_int")');
  assert.equal(removedMethodJson.artifactBytes.length, 0);
  assert.equal(removedMethodJson.diagnostics.length, 1);
  assert.equal(removedMethodJson.diagnostics[0].message, 'parser error: {error}: expected `json.json_get_int(key)` was removed; use `json.get_int(key)` but found Ident("json_get_int")');
  assert.equal(removedStdMap.artifactBytes.length, 0);
  assert.equal(removedStdMap.diagnostics.length, 1);
  assert.equal(removedStdMap.diagnostics[0].message, 'parser error: {error}: expected `contains(...)` was removed; use `map.contains(key)` but found Ident("std")');
  assert.equal(removedHostPath.artifactBytes.length, 0);
  assert.equal(removedHostPath.diagnostics.length, 1);
  assert.equal(removedHostPath.diagnostics[0].message, 'parser error: {error}: expected `path(...)` was removed as a free helper; use `base.path(segment)` but found Ident("host")');
});

test("Kotodama compiler SDK emits block_height as an extended sysvar syscall", () => {
  const compiled = compileKotodamaProgram(`
seiyaku BlockHeightSysvar {
  view fn height() -> int {
    return block_height();
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallxNeedle(0x01_0021)), -1);
  assert.equal(compiled.manifest?.entrypoints[0]?.return_type, "int");
});

test("Kotodama compiler SDK normalizes upstream host namespace helper calls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku HostNamespaceHelpers {
  kotoage fn run(payload: Json) -> bytes permission(Admin) {
    let path = host::name("HostNamespacePath");
    let stored = host::state_get(path);
    host::state_set(path, stored);
    host::state_del(path);
    let now = host::current_time_ms();
    info(now);
    return host::call_contract("target.contract", "settle", payload);
  }
}
`);
  const invalid = compileKotodamaProgram(`
seiyaku InvalidHostNamespaceHelper {
  kotoage fn run() permission(Admin) {
    let response = host::call_contract(1, "settle", json!{ amount: 1 });
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x50)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x51)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x52)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0xa8)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0xa9)), -1);
  assert.equal(compiled.manifest?.entrypoints[0]?.return_type, "bytes");
  assert.equal(invalid.artifactBytes.length, 0);
  assert.equal(invalid.diagnostics.length, 1);
  assert.match(invalid.diagnostics[0].message, /call_contract expects \(String\|Blob, String\|Blob, Json\)/);
});

test("Kotodama compiler SDK emits get_numeric through numeric getter and int conversion", () => {
  const compiled = compileKotodamaProgram(`
seiyaku JsonNumericGetter {
  fn amount() -> int {
    let payload = json!{ amount: 7 };
    return payload.get_numeric(name("amount"));
  }

  kotoage fn run() permission(Admin) {
    let value = amount();
    info(value);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x7f)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x6a)), -1);
});

test("Kotodama compiler SDK preserves numeric aliases and emits Numeric syscalls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku NumericAliases {
  fn parsed_amount() -> Amount {
    let payload = json!{ amount: 7 };
    let amount: Amount = payload.get_numeric(name("amount"));
    let next: Amount = amount + 1;
    return next;
  }

  view fn amount(value: Amount) -> Amount {
    return value + 1;
  }

  kotoage fn run() permission(Admin) {
    let amount: Amount = parsed_amount();
    let next: Amount = amount + 1;
    if next > amount {
      info(next);
    }
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(compiled.manifest?.entrypoints.find((entry) => entry.name === "amount")?.return_type, "Amount");
  assert.notEqual(code.indexOf(syscallNeedle(0x7f)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x69)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x6b)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x75)), -1);
});

test("Kotodama compiler SDK rejects invalid numeric alias operations", () => {
  const mixedAliases = compileKotodamaProgram(`
seiyaku MixedAliases {
  fn helper(amount: Amount, balance: Balance) {
    let bad = amount + balance;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const negativeAlias = compileKotodamaProgram(`
seiyaku NegativeAlias {
  fn helper() {
    let amount: Amount = -1;
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(mixedAliases.artifactBytes.length, 0);
  assert.equal(mixedAliases.diagnostics.length, 1);
  assert.match(mixedAliases.diagnostics[0].message, /Add expects int operands/);
  assert.equal(negativeAlias.artifactBytes.length, 0);
  assert.equal(negativeAlias.diagnostics.length, 1);
  assert.match(negativeAlias.diagnostics[0].message, /numeric alias literals must be unsigned/);
});

test("Kotodama compiler SDK emits NftId and Blob JSON payload getters", () => {
  const compiled = compileKotodamaProgram(`
seiyaku JsonPointerGetters {
  fn read_payload() {
    let payload = json!{ nft: "n0$wonderland.universal", proof: "010203" };
    let nft = payload.get_nft_id(name("nft"));
    let proof = payload.get_blob_hex(name("proof"));
    info(tlv_len(pointer_to_norito(nft)));
    info(tlv_len(proof));
  }

  kotoage fn run() permission(Admin) {
    read_payload();
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x7c)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x7d)), -1);
});

test("Kotodama compiler SDK wraps public NftId Blob and numeric entrypoint parameters", () => {
  const compiled = compileKotodamaProgram(`
seiyaku WrapperTypes {
  kotoage fn accept(nft: NftId, proof: Blob, amount: Amount) -> int permission(Admin) {
    info(tlv_len(pointer_to_norito(nft)));
    info(tlv_len(proof));
    let next: Amount = amount + 1;
    info(next);
    return 1;
  }
}
`);
  const direct = compileKotodamaProgram(`
seiyaku DirectWrapperTypes {
  kotoage fn accept(nft: NftId, proof: Blob, amount: Amount) -> int permission(Admin) {
    info(tlv_len(pointer_to_norito(nft)));
    info(tlv_len(proof));
    let next: Amount = amount + 1;
    info(next);
    return 1;
  }

  #[test]
  fn smoke() {
    let ok = invoke_entrypoint("accept", json("{\\"nft\\":\\"n0$wonderland.universal\\",\\"proof\\":\\"010203\\",\\"amount\\":7}"));
    assert_eq(ok, 1);
  }
}
`, { mode: "test" });
  const code = readArtifactCode(compiled.artifactBytes);
  const directCode = readArtifactCode(direct.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(direct.diagnostics, []);
  for (const syscall of [0x7c, 0x7d, 0x7f]) {
    assert.notEqual(code.indexOf(syscallNeedle(syscall)), -1, `missing wrapper syscall 0x${syscall.toString(16)}`);
    assert.notEqual(directCode.indexOf(syscallNeedle(syscall)), -1, `missing direct wrapper syscall 0x${syscall.toString(16)}`);
  }
  assert.ok(compiled.sourceMap.some((entry) => entry.function_name === "__entrypoint_impl__accept"));
  assert.ok(direct.sourceMap.some((entry) => entry.function_name === "smoke"));
});

test("Kotodama compiler SDK reuses dead public pointer parameters in nested return helpers", () => {
  const nameReturn = compileKotodamaProgram(`
seiyaku PublicNameReturn {
  kotoage fn run(label: Name) -> int {
    return tlv_len(pointer_to_norito(label));
  }
}
`);
  const accountReturn = compileKotodamaProgram(`
seiyaku PublicAccountReturn {
  kotoage fn run(id: AccountId) -> int {
    return tlv_len(pointer_to_norito(id));
  }
}
`);

  for (const compiled of [nameReturn, accountReturn]) {
    const implBudget = compiled.budgetReport.find((entry) => entry.function_name === "__entrypoint_impl__run");

    assert.deepEqual(compiled.diagnostics, []);
    assert.equal(compiled.artifactBytes.length, 997);
    assert.deepEqual(
      { bytecode_bytes: implBudget?.bytecode_bytes, frame_bytes: implBudget?.frame_bytes },
      { bytecode_bytes: 204, frame_bytes: 32 },
    );
  }
});

test("Kotodama compiler SDK reuses dead public parameters across private helper return branches", () => {
  const compiled = compileKotodamaProgram(`
seiyaku HelperReturnBranches {
  fn bump(x: int) -> int {
    return x + 1;
  }

  kotoage fn run(x: int) -> int {
    return bump(x) + bump(2);
  }
}
`);
  const implBudget = compiled.budgetReport.find((entry) => entry.function_name === "__entrypoint_impl__run");

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(compiled.artifactBytes.length, 1393);
  assert.deepEqual(
    { bytecode_bytes: implBudget?.bytecode_bytes, frame_bytes: implBudget?.frame_bytes },
    { bytecode_bytes: 516, frame_bytes: 40 },
  );
});

test("Kotodama compiler SDK passes single Json entrypoint payloads through whole", () => {
  const compiled = compileKotodamaProgram(`
seiyaku SingleJsonPayload {
  kotoage fn accept(payload: Json) -> bytes permission(Admin) {
    return call_contract("target.contract", "relay", payload);
  }
}
`);
  const direct = compileKotodamaProgram(`
seiyaku DirectSingleJsonPayload {
  kotoage fn accept(payload: Json) -> bytes permission(Admin) {
    return call_contract("target.contract", "relay", payload);
  }

  #[test]
  fn smoke() {
    let reply = invoke_entrypoint("accept", json("{\\"count\\":7}"));
    info(tlv_len(reply));
  }
}
`, { mode: "test" });
  const code = readArtifactCode(compiled.artifactBytes);
  const directCode = readArtifactCode(direct.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(direct.diagnostics, []);
  assert.equal(code.indexOf(syscallNeedle(0x79)), -1, "single Json wrapper must not JSON_GET_JSON by param name");
  assert.equal(directCode.indexOf(syscallNeedle(0x79)), -1, "direct test wrapper must not JSON_GET_JSON by param name");
  assert.notEqual(code.indexOf(syscallNeedle(0xa9)), -1, "body should still consume the Json payload");
  assert.notEqual(directCode.indexOf(syscallNeedle(0x57)), -1, "test wrapper should JSON_DECODE override payload");
  assert.ok(direct.sourceMap.some((entry) => entry.function_name === "smoke"));
});

test("Kotodama compiler SDK rejects unsupported public entrypoint parameters like Rust wrappers", () => {
  const compiled = compileKotodamaProgram(`
seiyaku BoolEntrypointParam {
  kotoage fn accept(flag: bool) -> int {
    if flag {
      return 1;
    }
    return 0;
  }
}
`);
  const direct = compileKotodamaProgram(`
seiyaku DirectBoolEntrypointParam {
  kotoage fn accept(flag: bool) -> int {
    if flag {
      return 1;
    }
    return 0;
  }

  #[test]
  fn smoke() {
    let got = invoke_entrypoint("accept", json("{\\"flag\\":true}"));
    assert_eq(got, 1);
  }
}
`, { mode: "test" });
  const mapParam = compileKotodamaProgram(`
seiyaku MapEntrypointParam {
  kotoage fn accept(values: Map<Name, int>) permission(Admin) {}
}
`);
  const tupleParam = compileKotodamaProgram(`
seiyaku TupleEntrypointParam {
  kotoage fn accept(value: (int, int)) permission(Admin) {}
}
`);
  const structParam = compileKotodamaProgram(`
seiyaku StructEntrypointParam {
  struct User { value: int }

  kotoage fn accept(value: User) permission(Admin) {}
}
`);
  const opaqueMapParam = compileKotodamaProgram(`
seiyaku OpaqueMapEntrypointParam {
  kotoage fn accept(values: Map<u32, int>) permission(Admin) {}
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.equal(
    compiled.diagnostics[0].message,
    "entrypoint parameter `flag` uses unsupported public type Bool",
  );
  assert.equal(direct.artifactBytes.length, 0);
  assert.equal(direct.diagnostics.length, 1);
  assert.equal(
    direct.diagnostics[0].message,
    "entrypoint parameter `flag` uses unsupported public type Bool",
  );
  assert.equal(
    mapParam.diagnostics[0].message,
    "entrypoint parameter `values` uses unsupported public type Map(Name, Int)",
  );
  assert.equal(
    tupleParam.diagnostics[0].message,
    "entrypoint parameter `value` uses unsupported public type Tuple([Int, Int])",
  );
  assert.equal(
    structParam.diagnostics[0].message,
    'entrypoint parameter `value` uses unsupported public type Struct { name: "User", fields: [("value", Int)] }',
  );
  assert.equal(
    opaqueMapParam.diagnostics[0].message,
    'entrypoint parameter `values` uses unsupported public type Map(Opaque("u32"), Int)',
  );
});

test("Kotodama compiler SDK mirrors Rust aggregate durable state type parsing", () => {
  const tupleState = compileKotodamaProgram(`
seiyaku TupleStateTypes {
  state Value: (int);
  state Values: Map<Name, (int)>;

  kotoage fn run() -> int {
    return 1;
  }
}
`);
  const structState = compileKotodamaProgram(`
seiyaku StructStateTypes {
  struct User { value: int, flag: bool }

  state Current: User;
  state Users: Map<Name, User>;

  kotoage fn run() -> int {
    return 1;
  }
}
`);
  const nestedMapState = compileKotodamaProgram(`
seiyaku NestedMapStateTypes {
  struct Holder { values: Map<Name, int>; }

  state Holder holder;

  kotoage fn run() -> int {
    return 1;
  }
}
`);
  const localTupleAnnotation = compileKotodamaProgram(`
seiyaku LocalSingleTupleAnnotation {
  kotoage fn run() -> int {
    let value: (int) = 1;
    return value;
  }
}
`);
  const publicTupleParam = compileKotodamaProgram(`
seiyaku PublicSingleTupleParam {
  kotoage fn run(value: (int)) permission(Admin) {}
}
`);
  const trailingCommaTuple = compileKotodamaProgram(`
seiyaku TrailingCommaTupleState {
  state Value: (int,);

  kotoage fn run() -> int {
    return 1;
  }
}
`);

  assert.deepEqual(tupleState.diagnostics, []);
  assert.deepEqual(tupleState.manifest?.states, [
    { name: "Value", type_name: "(int)" },
    { name: "Values", type_name: "map<Name, (int)>" },
  ]);
  assert.deepEqual(structState.diagnostics, []);
  assert.deepEqual(structState.manifest?.states, [
    { name: "Current", type_name: "User{value: int, flag: bool}" },
    { name: "Users", type_name: "map<Name, User{value: int, flag: bool}>" },
  ]);
  assert.deepEqual(nestedMapState.diagnostics, []);
  assert.deepEqual(nestedMapState.manifest?.states, [
    { name: "holder", type_name: "Holder{values: map<Name, int>}" },
  ]);
  assert.equal(localTupleAnnotation.artifactBytes.length, 0);
  assert.equal(
    localTupleAnnotation.diagnostics[0].message,
    "semantic error: type annotation mismatch: expected (int), got int",
  );
  assert.equal(publicTupleParam.artifactBytes.length, 0);
  assert.equal(
    publicTupleParam.diagnostics[0].message,
    "entrypoint parameter `value` uses unsupported public type Tuple([Int])",
  );
  assert.equal(trailingCommaTuple.artifactBytes.length, 0);
  assert.equal(
    trailingCommaTuple.diagnostics[0].message,
    "parser error: expected identifier but found RParen",
  );
});

test("Kotodama compiler SDK flattens scalar aggregate durable state reads and writes", () => {
  const compiled = compileKotodamaProgram(`
seiyaku StructAndState {
  struct Pair { first: int, second: int }

  state Pair stored_pair;

  kotoage fn set_pair(a: int, b: int) {
    stored_pair = Pair(a, b);
  }

  view fn sum_pair() -> int {
    return stored_pair.first + stored_pair.second;
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);
  let stateGetCount = 0;
  let stateSetCount = 0;
  for (let offset = 0; offset < code.length; offset += 1) {
    if (code.subarray(offset, offset + 4).equals(syscallNeedle(0x50))) {
      stateGetCount += 1;
    }
    if (code.subarray(offset, offset + 4).equals(syscallNeedle(0x51))) {
      stateSetCount += 1;
    }
  }
  const setPair = compiled.manifest?.entrypoints.find((entry) => entry.name === "set_pair");
  const sumPair = compiled.manifest?.entrypoints.find((entry) => entry.name === "sum_pair");

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(stateSetCount >= 2, "struct state assignment should write each scalar leaf");
  assert.ok(stateGetCount >= 2, "struct state reads should read each scalar leaf");
  assert.deepEqual(setPair?.write_keys, ["state:stored_pair_first", "state:stored_pair_second"]);
  assert.deepEqual(sumPair?.read_keys, ["state:stored_pair_first", "state:stored_pair_second"]);
  assert.deepEqual(compiled.manifest?.access_set_hints?.read_keys, [
    "state:stored_pair_first",
    "state:stored_pair_second",
  ]);
  assert.deepEqual(compiled.manifest?.access_set_hints?.write_keys, [
    "state:stored_pair_first",
    "state:stored_pair_second",
  ]);
});

test("Kotodama compiler SDK emits JSON object builder syscalls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku JsonObjectBuilders {
  kotoage fn build(owner: AccountId) -> Json {
    let payload = json_object();
    let payload = json_set_int(payload, name("bucket_id"), 1);
    return json_set_account_id(payload, name("owner"), owner);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x81)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x82)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x83)), -1);
  assert.equal(compiled.manifest?.entrypoints[0]?.return_type, "Json");
});

test("Kotodama compiler SDK accepts Rust int-like JSON integer builder values", () => {
  const compiled = compileKotodamaProgram(`
seiyaku JsonObjectIntLikeBuilders {
  kotoage fn build(amount: Amount, balance: Balance, exact: fixed_u128) -> Json {
    let payload = json_object();
    let payload = json_set_int(payload, name("amount"), amount);
    let payload = json_set_int(payload, name("balance"), balance);
    return json_set_int(payload, name("exact"), exact);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x6a)), -1, "missing NUMERIC_TO_INT syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x82)), -1, "missing JSON_SET_I64 syscall");
  assert.equal(compiled.manifest?.entrypoints[0]?.return_type, "Json");
  assert.equal(code.length, 1308);
  assert.equal(compiled.artifactBytes.length, 1981);
  assert.deepEqual(
    compiled.budgetReport.map(({ function_name, bytecode_bytes, frame_bytes }) => ({
      function_name,
      bytecode_bytes,
      frame_bytes,
    })),
    [
      { function_name: "build", bytecode_bytes: 620, frame_bytes: 48 },
      { function_name: "__entrypoint_impl__build", bytecode_bytes: 688, frame_bytes: 80 },
    ],
  );
});

test("Kotodama compiler SDK emits path helper syscalls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku PathHelpers {
  fn build_paths() {
    let base = name("EntryByKey");
    let int_path = base.path(7);
    let blob_path = base.path(norito_bytes("0x0102"));
    info(1);
  }

  kotoage fn run() {
    build_paths();
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x54)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x56)), -1);
});

test("Kotodama compiler SDK accepts Rust int-like path helper keys", () => {
  const compiled = compileKotodamaProgram(`
seiyaku NumericPathHelpers {
  fn build_paths(amount: Amount, balance: Balance, exact: fixed_u128) {
    let base = name("EntryByKey");
    let amount_path = base.path(amount);
    let balance_path = base.path(balance);
    let exact_path = base.path(exact);
    info(1);
  }

  kotoage fn run(amount: Amount, balance: Balance, exact: fixed_u128) permission(Admin) {
    build_paths(amount, balance, exact);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x54)), -1, "missing BUILD_PATH_MAP_KEY syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x6a)), -1, "missing NUMERIC_TO_INT syscall");
  assert.equal(code.indexOf(syscallNeedle(0x5c)), -1, "path base Name local should stay on the direct literal path");
  assert.equal(code.length, 1504);
  assert.equal(compiled.artifactBytes.length, 2273);
  assert.deepEqual(
    compiled.budgetReport.map(({ function_name, bytecode_bytes, frame_bytes }) => ({
      function_name,
      bytecode_bytes,
      frame_bytes,
    })),
    [
      { function_name: "run", bytecode_bytes: 612, frame_bytes: 48 },
      { function_name: "build_paths", bytecode_bytes: 500, frame_bytes: 72 },
      { function_name: "__entrypoint_impl__run", bytecode_bytes: 392, frame_bytes: 56 },
    ],
  );
});

test("Kotodama compiler SDK emits schema helper syscalls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku SchemaHelpers {
  fn roundtrip() -> Json {
    let schema = name("example.schema");
    let encoded = encode_schema(schema, json!{ ok: true });
    let decoded = decode_schema(schema, encoded);
    let info_json = schema_info(schema);
    info(1);
    return decoded;
  }

  kotoage fn run() permission(Admin) {
    let decoded = roundtrip();
    info(1);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x59)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x5a)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x5b)), -1);
});

test("Kotodama compiler SDK emits hash helper syscalls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku HashHelpers {
  fn digest(payload: Blob) {
    let sm3 = sm3_hash(payload);
    let sm3_namespaced = sm::hash(payload);
    let sm3_explicit_namespaced = sm::sm3_hash(payload);
    let sha256 = sha256_hash(payload);
    let sha3 = sha3_hash(payload);
    info(tlv_len(sm3));
    info(tlv_len(sm3_namespaced));
    info(tlv_len(sm3_explicit_namespaced));
    info(tlv_len(sha256));
    info(tlv_len(sha3));
  }

  kotoage fn run() permission(Admin) {
    digest(blob("0x010203"));
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x90)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x96)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x97)), -1);
});

test("Kotodama compiler SDK rejects stale free hash helper aliases", () => {
  const compiled = compileKotodamaProgram(`
seiyaku FreeHashAlias {
  kotoage fn run() permission(Admin) {
    let digest = hash(blob("0x010203"));
    info(1);
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.match(compiled.diagnostics[0].message, /hash/);
});

test("Kotodama compiler SDK emits namespaced ZK helper syscalls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ZkNamespaceHelpers {
  fn verify(payload: Blob) {
    zk::verify_transfer(payload);
    zk::verify_unshield(payload);
    zk::verify_batch(payload);
    zk::vote::verify_ballot(payload);
    zk::vote::verify_tally(payload);
  }

  kotoage fn run() permission(Admin) {
    verify(blob("0x010203"));
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x60)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x61)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x68)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x62)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x63)), -1);
});

test("Kotodama compiler SDK emits encode and decode helper syscalls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku EncodeDecodeHelpers {
  fn roundtrip() -> int {
    let int_bytes = encode_int(7);
    let decoded_int = decode_int(int_bytes);
    let json_bytes = encode_json(json!{ ok: true });
    let decoded_json = decode_json(json_bytes);
    info(tlv_len(json_bytes));
    return decoded_int;
  }

  kotoage fn run() permission(Admin) {
    let decoded = roundtrip();
    info(decoded);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x55)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x53)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x57)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x58)), -1);
});

test("Kotodama compiler SDK accepts Rust int-like encode_int values", () => {
  const compiled = compileKotodamaProgram(`
seiyaku EncodeIntLikeHelpers {
  fn encode(amount: Amount, balance: Balance, exact: fixed_u128) {
    let amount_bytes = encode_int(amount);
    let balance_bytes = encode_int(balance);
    let exact_bytes = encode_int(exact);
    info(tlv_len(amount_bytes) + tlv_len(balance_bytes) + tlv_len(exact_bytes));
  }

  kotoage fn run(amount: Amount, balance: Balance, exact: fixed_u128) permission(Admin) {
    encode(amount, balance, exact);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x55)), -1, "missing ENCODE_INT syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x6a)), -1, "missing NUMERIC_TO_INT syscall");
});

test("Kotodama compiler SDK emits direct durable state helper syscalls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku DirectStateHelpers {
  fn touch() {
    let path = name("DirectStateValue");
    let stored = state_get(path);
    state_set(path, stored);
    state_del(path);
  }

  kotoage fn run() permission(Admin) {
    touch();
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x50)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x51)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x52)), -1);
});

test("Kotodama compiler SDK emits call_contract with bytes aliases", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ContractCallRelay {
  kotoage fn relay(target: bytes, payload: Json) -> bytes permission(Admin) {
    return call_contract(target, "settle", payload);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0xa9)), -1);
  assert.equal(compiled.manifest?.entrypoints[0]?.return_type, "bytes");
  assert.equal(compiled.manifest?.entrypoints[0]?.access_hints_complete, false);
});

test("Kotodama compiler SDK renders canonical bytes state and ABI type names", () => {
  const compiled = compileKotodamaProgram(`
seiyaku CanonicalBytesTypes {
  state Payload: bytes;
  state Values: Map<Name, bytes>;

  kotoage fn run(key: Name, payload: Bytes) -> bytes permission(Admin) {
    Payload = payload;
    Values[key] = Payload;
    return Values[key];
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.params, [
    { name: "key", type_name: "Name" },
    { name: "payload", type_name: "bytes" },
  ]);
  assert.equal(compiled.manifest?.entrypoints[0]?.return_type, "bytes");
  assert.deepEqual(compiled.manifest?.states, [
    { name: "Payload", type_name: "bytes" },
    { name: "Values", type_name: "map<Name, bytes>" },
  ]);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ({
      function_name: entry.function_name,
      bytecode_bytes: entry.bytecode_bytes,
      frame_bytes: entry.frame_bytes,
      pc_start: entry.pc_start,
      pc_end: entry.pc_end,
    })),
    [
      { function_name: "run", bytecode_bytes: 488, frame_bytes: 40, pc_start: 0, pc_end: 488 },
      { function_name: "__entrypoint_impl__run", bytecode_bytes: 748, frame_bytes: 64, pc_start: 488, pc_end: 1236 },
    ],
  );

  const scalarState = compileKotodamaProgram(`
seiyaku ScalarBytesState {
  state Payload: bytes;

  kotoage fn run(payload: bytes) -> bytes permission(Admin) {
    Payload = payload;
    return Payload;
  }
}
`);

  assert.deepEqual(scalarState.diagnostics, []);
  assert.deepEqual(
    scalarState.budgetReport.map((entry) => ({
      function_name: entry.function_name,
      bytecode_bytes: entry.bytecode_bytes,
      frame_bytes: entry.frame_bytes,
      pc_start: entry.pc_start,
      pc_end: entry.pc_end,
    })),
    [
      { function_name: "run", bytecode_bytes: 356, frame_bytes: 32, pc_start: 0, pc_end: 356 },
      { function_name: "__entrypoint_impl__run", bytecode_bytes: 328, frame_bytes: 40, pc_start: 356, pc_end: 684 },
    ],
  );
});

test("Kotodama compiler SDK treats user-written NoritoBytes as a normal struct name", () => {
  const publicType = compileKotodamaProgram(`
seiyaku PublicNoritoBytes {
  kotoage fn run(payload: NoritoBytes) -> NoritoBytes permission(Admin) {
    return payload;
  }
}
`);
  const stateType = compileKotodamaProgram(`
seiyaku StateNoritoBytes {
  state Payload: NoritoBytes;

  kotoage fn run(payload: bytes) -> bytes permission(Admin) {
    Payload = payload;
    return Payload;
  }
}
`);
  const mapStateType = compileKotodamaProgram(`
seiyaku MapStateNoritoBytes {
  state Values: Map<Name, NoritoBytes>;

  kotoage fn run(key: Name, payload: bytes) -> bytes permission(Admin) {
    Values[key] = payload;
    return Values[key];
  }
}
`);
  const localType = compileKotodamaProgram(`
seiyaku LocalNoritoBytes {
  kotoage fn run(payload: bytes) -> bytes permission(Admin) {
    let local: NoritoBytes = payload;
    return local;
  }
}
`);
  const userStruct = compileKotodamaProgram(`
seiyaku StructNoritoBytes {
  struct NoritoBytes { value: int }

  fn make() -> NoritoBytes {
    return NoritoBytes(1);
  }

  view fn run() -> int {
    let value = make();
    return value.value;
  }
}
`);

  assert.equal(publicType.artifactBytes.length, 0);
  assert.equal(
    publicType.diagnostics[0].message,
    'entrypoint parameter `payload` uses unsupported public type Opaque("NoritoBytes")',
  );
  assert.equal(stateType.artifactBytes.length, 0);
  assert.equal(
    stateType.diagnostics[0].message,
    'semantic error: state type `NoritoBytes` is not supported for durable storage; use int, bool, Json, Blob, or pointer types',
  );
  assert.equal(mapStateType.artifactBytes.length, 0);
  assert.equal(
    mapStateType.diagnostics[0].message,
    'semantic error: state Map value type `NoritoBytes` is not supported for durable storage; use int, bool, Json, Blob, or pointer types',
  );
  assert.equal(localType.artifactBytes.length, 0);
  assert.equal(
    localType.diagnostics[0].message,
    'semantic error: type annotation mismatch: expected NoritoBytes, got bytes',
  );
  assert.deepEqual(userStruct.diagnostics, []);
  assert.equal(userStruct.manifest?.entrypoints[0]?.return_type, "int");
});

test("Kotodama compiler SDK mirrors Rust numeric type aliases", () => {
  const aliases = compileKotodamaProgram(`
seiyaku NumericTypeAliases {
  state Counter: i64;
  state Values: Map<number, i64>;

  kotoage fn run(key: i64, value: number) -> i64 permission(Admin) {
    Counter = value;
    Values[key] = Counter;
    let local: number = Values[key];
    return local;
  }
}
`);
  const staleU32 = compileKotodamaProgram(`
seiyaku StaleU32Alias {
  kotoage fn run(value: u32) -> u32 permission(Admin) {
    return value;
  }
}
`);
  const staleNumeric = compileKotodamaProgram(`
seiyaku StaleNumericAlias {
  kotoage fn run(value: Numeric) -> Numeric permission(Admin) {
    return value;
  }
}
`);

  assert.deepEqual(aliases.diagnostics, []);
  assert.deepEqual(aliases.manifest?.entrypoints[0]?.params, [
    { name: "key", type_name: "int" },
    { name: "value", type_name: "int" },
  ]);
  assert.equal(aliases.manifest?.entrypoints[0]?.return_type, "int");
  assert.deepEqual(aliases.manifest?.states, [
    { name: "Counter", type_name: "int" },
    { name: "Values", type_name: "map<int, int>" },
  ]);
  assert.equal(staleU32.artifactBytes.length, 0);
  assert.equal(
    staleU32.diagnostics[0].message,
    'entrypoint parameter `value` uses unsupported public type Opaque("u32")',
  );
  assert.equal(staleNumeric.artifactBytes.length, 0);
  assert.equal(
    staleNumeric.diagnostics[0].message,
    'entrypoint parameter `value` uses unsupported public type Opaque("Numeric")',
  );
});

test("Kotodama compiler SDK preserves dynamic String ABI values", () => {
  const compiled = compileKotodamaProgram(`
seiyaku DynamicStringAbi {
  fn label(raw: String) -> String {
    return raw;
  }

  view fn current_label() -> String {
    let raw: String = "settle";
    return label(raw);
  }

  kotoage fn run() permission(Admin) {
    let asset = asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    let symbol: String = label("ROSE");
    register_asset(asset, symbol, 0, 1);
    create_new_asset(asset, symbol, 7, authority(), 0);
    let response = call_contract("contract", label("settle"), json!{ ok: true });
    info(tlv_len(response));
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(
    compiled.manifest?.entrypoints.find((entry) => entry.name === "current_label")?.return_type,
    "string",
  );
  assert.notEqual(code.indexOf(syscallNeedle(0x20)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x22)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0xa9)), -1);
});

test("Kotodama compiler SDK emits account and asset lifecycle syscalls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku LifecycleHelpers {
  kotoage fn run() permission(Admin) {
    let asset = asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    register_account(authority());
    unregister_account(authority());
    register_asset(asset, "ROSE", 0, 1);
    create_new_asset(asset, "ROSE", 7, authority(), 0);
    unregister_asset(asset);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x13)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x14)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x20)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x21)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x22)), -1);
});

test("Kotodama compiler SDK emits role and permission syscalls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku RolePermissionHelpers {
  kotoage fn run() permission(Admin) {
    let role = name("auditor");
    create_role(role, json!{ permissions: ["read_blocks"] });
    grant_role(authority(), role);
    revoke_role(authority(), role);
    grant_permission(authority(), name("read_blocks"));
    revoke_permission(authority(), json!{ "permission": "read_blocks" });
    delete_role(role);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x30)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x31)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x32)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x33)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x34)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x35)), -1);
});

test("Kotodama compiler SDK emits peer and trigger management syscalls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku PeerTriggerHelpers {
  kotoage fn run() permission(Admin) {
    let trigger = name("wake");
    register_peer(json!{ address: "127.0.0.1:1337" });
    unregister_peer(json!{ address: "127.0.0.1:1337" });
    create_trigger(json!{ id: "wake", action: "noop" });
    remove_trigger(trigger);
    set_trigger_enabled(trigger, 1);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x15)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x16)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x40)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x41)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x42)), -1);
});

test("Kotodama compiler SDK emits VRF helper syscalls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku VrfHelpers {
  fn verify(payload: Blob) {
    let proof = vrf_verify(payload, payload, payload, 1);
    let batch = vrf_verify_batch(payload);
    info(1);
  }

  kotoage fn run() permission(Admin) {
    verify(blob("0x010203"));
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x66)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x67)), -1);
});

test("Kotodama compiler SDK emits signature verification helper syscalls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku SignatureHelpers {
  fn verify(payload: Blob) {
    let sm2 = sm2_verify(payload, payload, payload);
    let sm2_with_distid = sm2_verify(payload, payload, payload, payload);
    let generic = verify_signature(payload, payload, payload, 0);
    assert(sm2, "sm2");
    assert(sm2_with_distid, "sm2_distid");
    assert(generic, "generic");
  }

  kotoage fn run() permission(Admin) {
    verify(blob("0x010203"));
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x91)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0xfc)), -1);
});

test("Kotodama compiler SDK emits SM4 helper syscalls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku Sm4Helpers {
  fn crypt(payload: Blob) {
    let gcm = sm4_gcm_seal(payload, payload, payload, payload);
    let opened_gcm = sm4_gcm_open(payload, payload, payload, gcm);
    let ccm = sm::seal_ccm(payload, payload, payload, payload, 12);
    let opened_ccm = sm::open_ccm(payload, payload, payload, ccm);
    info(tlv_len(opened_gcm));
    info(tlv_len(opened_ccm));
  }

  kotoage fn run() permission(Admin) {
    crypt(blob("0x010203"));
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x92)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x93)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x94)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x95)), -1);
});

test("Kotodama compiler SDK accepts Rust int-like crypto selector arguments", () => {
  const compiled = compileKotodamaProgram(`
seiyaku CryptoIntLikeSelectors {
  fn verify(payload: Blob, amount: Amount, balance: Balance, exact: fixed_u128) {
    let vrf = vrf_verify(payload, payload, payload, amount);
    let signature = verify_signature(payload, payload, payload, balance);
    let sealed = sm4_ccm_seal(payload, payload, payload, payload, exact);
    let opened = sm4_ccm_open(payload, payload, payload, sealed, amount);
    assert(signature, "signature");
    info(tlv_len(vrf) + tlv_len(opened));
  }

  kotoage fn run(amount: Amount, balance: Balance, exact: fixed_u128) permission(Admin) {
    verify(blob("0x010203"), amount, balance, exact);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x66)), -1, "missing VRF_VERIFY syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0xfc)), -1, "missing VERIFY_SIGNATURE syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x94)), -1, "missing SM4_CCM_SEAL syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x95)), -1, "missing SM4_CCM_OPEN syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x6a)), -1, "missing NUMERIC_TO_INT syscall");
});

test("Kotodama compiler SDK emits map enumeration helper loads", () => {
  const compiled = compileKotodamaProgram(`
seiyaku MapEnumerationHelpers {
  fn scan() -> int {
    let values: Map<int, int> = std::map::new();
    let mirror: Map<int, int> = std::Map::new();
    values[11] = 22;
    mirror[33] = 44;
    let key = keys_take2(values, 0, 0);
    let value = std::map::values_take2(values, 0, 0);
    let pair = keys_values_take2(values, 0, 0);
    return key + value + pair.0 + pair.1 + mirror[33];
  }

  kotoage fn run() permission(Admin) {
    let value = scan();
    info(value);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);
  let load64Count = 0;
  for (let offset = 3; offset < code.length; offset += 4) {
    if (code[offset] === 0x30) {
      load64Count += 1;
    }
  }

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(load64Count >= 6);
});

test("Kotodama compiler SDK preserves Rust bytecode shape for direct map enumeration returns", () => {
  const compiled = compileKotodamaProgram(`
seiyaku MapEnum {
  kotoage fn run() -> int permission(Admin) {
    let values: Map<int, int> = Map::new();
    values[11] = 22;
    let first_key = keys_take2(values, 0, 0);
    let first_value = std::map::values_take2(values, 0, 0);
    let first_pair = keys_values_take2(values, 0, 0);
    return first_key + first_value + first_pair.0 + first_pair.1;
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);
  const runBudget = compiled.budgetReport.find((entry) => entry.function_name === "run");

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(runBudget?.bytecode_bytes, 336);
  assert.equal(runBudget?.frame_bytes, 72);
  assert.equal(runBudget?.bytecode_words, 84);
  assert.equal(code.length, 336);
  assert.equal(
    Buffer.from(code.subarray(208, 328)).toString("hex"),
    "000005200000072000000620010606200607170300170620000017201017172017060510051817010017053008170630000507200000172000000620000005200105052005060403000405200000042010040420040517101718040100041730080418300018052008090401070408010508040100040a20",
  );
});

test("Kotodama compiler SDK parses bounded upstream map foreach syntax", () => {
  const compiled = compileKotodamaProgram(`
seiyaku BoundedMapForeach {
  fn scan_attr() -> int {
    let values: Map<int, int> = Map::new();
    values[11] = 22;
    let total = 0;
    for (key, value) in values #[bounded(1)] {
      total = total + key + value;
    }
    return total;
  }

  fn scan_take() -> int {
    let values: Map<int, int> = Map::new();
    values[3] = 5;
    let total = 0;
    for (key, value) in values.take(1) {
      total = total + key + value;
    }
    return total;
  }

  fn scan_range() -> int {
    let values: Map<int, int> = Map::new();
    values[7] = 9;
    let total = 0;
    for key in values.range(0, 1) {
      total = total + key;
    }
    return total;
  }

  kotoage fn run() permission(Admin) {
    info(scan_attr() + scan_take() + scan_range());
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);
  let load64Count = 0;
  for (let offset = 3; offset < code.length; offset += 4) {
    if (code[offset] === 0x30) {
      load64Count += 1;
    }
  }

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(load64Count >= 6);
});

test("Kotodama compiler SDK supports direct map-valued helper foreach expressions", () => {
  const compiled = compileKotodamaProgram(`
seiyaku DirectMapForeach {
  fn make() -> Map<int, int> {
    let values: Map<int, int> = Map::new();
    values[3] = 5;
    return values;
  }

  kotoage fn run() -> int permission(Admin) {
    let total = 0;
    for (key, value) in make().take(1) {
      total = total + key + value;
    }
    return total;
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(compiled.codeHashHex.length, 64);
  assert.ok(compiled.artifactBytes.length > 64);
});

test("Kotodama compiler SDK compiles bounded in-memory map foreach with policy-supported word keys", () => {
  const compiled = compileKotodamaProgram(`
seiyaku LocalWordMapForeach {
  fn scan_name() -> int {
    let values: Map<Name, int> = Map::new();
    values[name("alice")] = 7;
    let total = 0;
    for (key, value) in values #[bounded(1)] {
      total = total + value + tlv_len(pointer_to_norito(key));
    }
    return total;
  }

  kotoage fn run() permission(Admin) {
    info(scan_name());
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0xe0)), -1, "word-key map foreach should publish key TLVs when used");
  assert.notEqual(code.indexOf(syscallNeedle(0x77)), -1, "word-key map foreach should support TLV length reads");
  assert.ok(compiled.artifactBytes.length > 64);
});

test("Kotodama compiler SDK compiles bounded durable state map foreach loops", () => {
  const compiled = compileKotodamaProgram(`
seiyaku StateMapForeach {
  state Values: Map<int, int>;

  fn scan_attr() -> int {
    let total = 0;
    for (key, value) in Values #[bounded(2)] {
      total = total + key + value;
    }
    return total;
  }

  fn scan_take() -> int {
    let total = 0;
    for (key, value) in Values.take(2) {
      total = total + key + value;
    }
    return total;
  }

  fn scan_range() -> int {
    let total = 0;
    for (key, value) in Values.range(1, 3) {
      total = total + key + value;
    }
    return total;
  }

  kotoage fn run() permission(Admin) {
    Values[0] = 2;
    Values[1] = 3;
    Values[2] = 5;
    info(scan_attr() + scan_take() + scan_range());
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x50)), -1, "missing STATE_GET syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x51)), -1, "missing STATE_SET syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x53)), -1, "missing DECODE_INT syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x54)), -1, "missing BUILD_PATH_MAP_KEY syscall");
});

test("Kotodama compiler SDK compiles dynamic durable state map foreach bounds", () => {
  const compiled = compileKotodamaProgram(`
seiyaku DynamicStateMapForeach {
  state Values: Map<int, int>;

  fn scan_take(n: int) -> int {
    let total = 0;
    for (key, value) in Values.take(n) {
      total = total + key + value;
    }
    return total;
  }

  fn scan_range(start: int, end: int) -> int {
    let total = 0;
    for (key, value) in Values.range(start, end) {
      total = total + key + value;
    }
    return total;
  }

  kotoage fn run(n: int, start: int, end: int) -> int permission(Admin) {
    return scan_take(n) + scan_range(start, end);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x02)), -1, "missing dynamic-bound ASSERT abort syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x50)), -1, "missing STATE_GET syscall");
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.read_keys, []);
  assert.equal(compiled.manifest?.entrypoints[0]?.access_hints_complete, true);
  assert.deepEqual(compiled.manifest?.access_set_hints?.read_keys, ["state:Values"]);
  assert.deepEqual(compiled.manifest?.access_set_hints?.dynamic_reads, [
    {
      base_key: "state:Values",
      key_type: "int",
      bound_kind: "range",
      max_keys: 64,
    },
    {
      base_key: "state:Values",
      key_type: "int",
      bound_kind: "take",
      max_keys: 64,
    },
  ]);
  assert.equal(
    code.subarray(736, 760).toString("hex"),
    "000009200009072000180820000009200109092019000046",
  );
  assert.equal(
    code.subarray(1172, 1216).toString("hex"),
    "e000006000070b2054000060000a062000060a20e000006050000060000a1820000006200618050f1a000541",
  );
  assert.equal(
    code.subarray(1416, 1444).toString("hex"),
    "00180a20e000006053000060000a0520000718200018062005061801",
  );
  assert.equal(code.subarray(1544, 1552).toString("hex"), "0907070153ff0046");
  assert.equal(
    code.subarray(1884, 1908).toString("hex"),
    "181707020018172007180501000007200107072019000046",
  );
  assert.equal(
    code.subarray(2320, 2364).toString("hex"),
    "e000006000170b2054000060000a062000060a20e000006050000060000a1820000006200618080f1a000841",
  );
  assert.equal(
    code.subarray(2564, 2592).toString("hex"),
    "00180a20e000006053000060000a0820001718200018062008061801",
  );
  assert.equal(code.subarray(2692, 2700).toString("hex"), "0717170153ff0046");
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ({
      function_name: entry.function_name,
      pc_start: entry.pc_start,
      pc_end: entry.pc_end,
      bytecode_bytes: entry.bytecode_bytes,
      frame_bytes: entry.frame_bytes,
    })),
    [
      { function_name: "run", pc_start: 0, pc_end: 620, bytecode_bytes: 620, frame_bytes: 48 },
      { function_name: "scan_take", pc_start: 620, pc_end: 1732, bytecode_bytes: 1112, frame_bytes: 72 },
      { function_name: "scan_range", pc_start: 1732, pc_end: 2880, bytecode_bytes: 1148, frame_bytes: 80 },
      {
        function_name: "__entrypoint_impl__run",
        pc_start: 2880,
        pc_end: 3524,
        bytecode_bytes: 644,
        frame_bytes: 64,
      },
    ],
  );
});

test("Kotodama compiler SDK reports direct dynamic durable state map entrypoint reads", () => {
  const compiled = compileKotodamaProgram(`
seiyaku DirectDynamicStateMapForeach {
  state Values: Map<int, int>;

  kotoage fn run(n: int) -> int permission(Admin) {
    let total = 0;
    for (key, value) in Values.take(n) {
      total = total + key + value;
    }
    return total;
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.read_keys, ["state:Values"]);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.write_keys, []);
  assert.deepEqual(compiled.manifest?.access_set_hints, {
    read_keys: ["state:Values"],
    write_keys: [],
    dynamic_reads: [
      {
        base_key: "state:Values",
        key_type: "int",
        bound_kind: "take",
        max_keys: 64,
      },
    ],
    dynamic_writes: [],
  });
});

test("Kotodama compiler SDK compiles aggregate durable state map foreach values", () => {
  const compiled = compileKotodamaProgram(`
seiyaku AggregateStateMapForeach {
  struct Pair {
    left: int,
    right: int,
  }

  state StructValues: Map<int, Pair>;
  state TupleValues: Map<int, (int, int)>;

  fn scan_structs() -> int {
    let total = 0;
    for (key, value) in StructValues #[bounded(2)] {
      total = total + key + value.left + value.right;
    }
    return total;
  }

  fn scan_tuples() -> int {
    let total = 0;
    for (key, value) in TupleValues.take(2) {
      total = total + key + value.0 + value.1;
    }
    return total;
  }

  kotoage fn run() permission(Admin) {
    StructValues[0] = Pair(2, 3);
    TupleValues[0] = (5, 7);
    info(scan_structs() + scan_tuples());
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);
  let stateGetCount = 0;
  for (let offset = 0; offset < code.length; offset += 1) {
    if (code.subarray(offset, offset + 4).equals(syscallNeedle(0x50))) {
      stateGetCount += 1;
    }
  }

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(stateGetCount >= 4, "aggregate foreach should read flattened state-map fields");
  assert.notEqual(code.indexOf(syscallNeedle(0x51)), -1, "missing STATE_SET syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x53)), -1, "missing DECODE_INT syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x54)), -1, "missing BUILD_PATH_MAP_KEY syscall");
});

test("Kotodama compiler SDK compiles aggregate state map helper handles", () => {
  const compiled = compileKotodamaProgram(`
seiyaku AggregateStateMapHandles {
  struct Pair {
    left: int,
    right: int,
  }

  state Pairs: Map<int, Pair>;
  state Tuples: Map<int, (int, int)>;

  fn ensure_pair(state Map<int, Pair> values, key: int) -> Pair {
    return values.ensure(key, Pair(2, 3));
  }

  fn sum_pair(state Map<int, Pair> values, key: int) -> int {
    let value = values.get_or(key, Pair(5, 7));
    return value.left + value.right;
  }

  fn forward_pair(state Map<int, Pair> values, key: int) -> int {
    return sum_pair(values, key);
  }

  fn ensure_tuple(state Map<int, (int, int)> values, key: int) -> (int, int) {
    return values.ensure(key, (11, 13));
  }

  kotoage fn run() permission(Admin) {
    let pair = ensure_pair(Pairs, 0);
    let tuple = ensure_tuple(Tuples, 1);
    info(pair.left + pair.right + tuple.0 + tuple.1 + forward_pair(Pairs, 0));
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);
  let stateGetCount = 0;
  let stateSetCount = 0;
  for (let offset = 0; offset < code.length; offset += 1) {
    if (code.subarray(offset, offset + 4).equals(syscallNeedle(0x50))) {
      stateGetCount += 1;
    }
    if (code.subarray(offset, offset + 4).equals(syscallNeedle(0x51))) {
      stateSetCount += 1;
    }
  }

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(stateGetCount >= 4, "aggregate state-map handles should read flattened fields");
  assert.ok(stateSetCount >= 4, "aggregate state-map handles should write flattened fields");
  assert.notEqual(code.indexOf(syscallNeedle(0x53)), -1, "missing DECODE_INT syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x54)), -1, "missing BUILD_PATH_MAP_KEY syscall");
});

test("Kotodama compiler SDK rejects unsupported map foreach bounds and mutations", () => {
  const unbounded = compileKotodamaProgram(`
seiyaku UnboundedMapForeach {
  fn scan() -> int {
    let values: Map<int, int> = Map::new();
    let total = 0;
    for (key, value) in values {
      total = total + key + value;
    }
    return total;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const tooWide = compileKotodamaProgram(`
seiyaku TooWideMapForeach {
  fn scan() -> int {
    let values: Map<int, int> = Map::new();
    let total = 0;
    for (key, value) in values #[bounded(2)] {
      total = total + key + value;
    }
    return total;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidRange = compileKotodamaProgram(`
seiyaku InvalidRangeMapForeach {
  fn scan() -> int {
    let values: Map<int, int> = Map::new();
    let total = 0;
    for key in values.range(1, 0) {
      total = total + key;
    }
    return total;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const localRangeStart = compileKotodamaProgram(`
seiyaku LocalRangeStartMapForeach {
  fn scan() -> int {
    let values: Map<int, int> = Map::new();
    let total = 0;
    for (key, value) in values.range(1, 2) {
      total = total + key + value;
    }
    return total;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const dynamicLocal = compileKotodamaProgram(`
seiyaku DynamicLocalMapForeach {
  fn scan(n: int) -> int {
    let values: Map<int, int> = Map::new();
    let total = 0;
    for (key, value) in values.take(n) {
      total = total + key + value;
    }
    return total;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const dynamicStateWrongBound = compileKotodamaProgram(`
seiyaku DynamicStateWrongBound {
  state Values: Map<int, int>;

  kotoage fn run(bound: Name) permission(Admin) {
    for (key, value) in Values.take(bound) {
      info(key + value);
    }
  }
}
`);
  const nonIntStateKey = compileKotodamaProgram(`
seiyaku NonIntStateMapForeach {
  state Values: Map<Name, int>;

  kotoage fn run() permission(Admin) {
    let total = 0;
    for (key, value) in Values #[bounded(1)] {
      total = total + value;
    }
    info(total);
  }
}
`);
  const nestedNonIntStateKey = compileKotodamaProgram(`
seiyaku NestedNonIntStateMapForeach {
  struct Holder { values: Map<Name, int>; }
  state Holder holder;

  kotoage fn run() permission(Admin) {
    for (key, value) in holder.values #[bounded(1)] {
      info(value);
    }
  }
}
`);
  const mutation = compileKotodamaProgram(`
seiyaku MutatingMapForeach {
  fn scan() -> int {
    let values: Map<int, int> = Map::new();
    values[1] = 2;
    for (key, value) in values #[bounded(1)] {
      values[key] = value + 1;
    }
    return values[1];
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(unbounded.artifactBytes.length, 0);
  assert.equal(unbounded.diagnostics.length, 1);
  assert.match(unbounded.diagnostics[0].message, /E_UNBOUNDED_ITERATION/);
  assert.equal(tooWide.artifactBytes.length, 0);
  assert.equal(tooWide.diagnostics.length, 1);
  assert.match(tooWide.diagnostics[0].message, /E_MAP_BOUNDS/);
  assert.equal(invalidRange.artifactBytes.length, 0);
  assert.equal(invalidRange.diagnostics.length, 1);
  assert.match(invalidRange.diagnostics[0].message, /end >= start/);
  assert.equal(localRangeStart.artifactBytes.length, 0);
  assert.equal(localRangeStart.diagnostics.length, 1);
  assert.match(localRangeStart.diagnostics[0].message, /starting at index 0/);
  assert.equal(dynamicLocal.artifactBytes.length, 0);
  assert.equal(dynamicLocal.diagnostics.length, 1);
  assert.match(dynamicLocal.diagnostics[0].message, /dynamic bounds on in-memory Map iteration are unsupported/);
  assert.equal(dynamicStateWrongBound.artifactBytes.length, 0);
  assert.equal(dynamicStateWrongBound.diagnostics.length, 1);
  assert.match(dynamicStateWrongBound.diagnostics[0].message, /`\.take\(n\)` requires an integer bound/);
  assert.equal(nonIntStateKey.artifactBytes.length, 0);
  assert.equal(nonIntStateKey.diagnostics.length, 1);
  assert.match(nonIntStateKey.diagnostics[0].message, /durable state map iteration supports Map<int, \*> keys only/);
  assert.equal(nestedNonIntStateKey.artifactBytes.length, 0);
  assert.equal(nestedNonIntStateKey.diagnostics.length, 1);
  assert.match(nestedNonIntStateKey.diagnostics[0].message, /durable state map iteration supports Map<int, \*> keys only/);
  assert.equal(mutation.artifactBytes.length, 0);
  assert.equal(mutation.diagnostics.length, 1);
  assert.match(mutation.diagnostics[0].message, /E_ITER_MUTATION/);
});

test("Kotodama compiler SDK mirrors Rust bounded map attribute parser diagnostics", () => {
  const compileBounded = (attribute) => compileKotodamaProgram(`
seiyaku BoundedAttributeDiagnostics {
  state Values: Map<int, int>;

  kotoage fn run() permission(Admin) {
    for key in Values ${attribute} {
      info(key);
    }
  }
}
`);
  const badName = compileBounded("#[limit(1)]");
  const badIdentifier = compileBounded("#[123(1)]");
  const badValue = compileBounded('#[bounded("1")]');
  const missingRightParen = compileBounded("#[bounded(1]");
  const missingRightBracket = compileBounded("#[bounded(1) {");

  assert.equal(badName.artifactBytes.length, 0);
  assert.equal(badName.diagnostics[0].message, 'parser error: {error}: expected expected attribute `bounded` but found Ident("limit")');
  assert.equal(badIdentifier.artifactBytes.length, 0);
  assert.equal(badIdentifier.diagnostics[0].message, "parser error: {error}: expected expected attribute identifier but found Number(123)");
  assert.equal(badValue.artifactBytes.length, 0);
  assert.equal(badValue.diagnostics[0].message, 'parser error: {error}: expected `bounded(n)` expects a non-negative integer literal but found String("1")');
  assert.equal(missingRightParen.artifactBytes.length, 0);
  assert.equal(missingRightParen.diagnostics[0].message, "parser error: {error}: expected RParen but found RBracket");
  assert.equal(missingRightBracket.artifactBytes.length, 0);
  assert.equal(missingRightBracket.diagnostics[0].message, "parser error: {error}: expected RBracket but found LBrace");
});

test("Kotodama compiler SDK rejects invalid host helper call arguments semantically", () => {
  const invalidExecute = compileKotodamaProgram(`
seiyaku InvalidExecuteInstruction {
  fn helper() {
    execute_instruction(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidZk = compileKotodamaProgram(`
seiyaku InvalidZkVerify {
  fn helper() {
    zk::verify_unshield(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidStateGet = compileKotodamaProgram(`
seiyaku InvalidStateGet {
  fn helper() {
    let value = state_get(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidStateSet = compileKotodamaProgram(`
seiyaku InvalidStateSet {
  fn helper() {
    state_set(name("path"), 1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidStateDel = compileKotodamaProgram(`
seiyaku InvalidStateDel {
  fn helper() {
    state_del(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidDepth = compileKotodamaProgram(`
seiyaku InvalidDepth {
  fn helper() {
    set_execution_depth("deep");
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidSetvlDynamic = compileKotodamaProgram(`
seiyaku InvalidSetvlDynamic {
  fn helper(value: int) {
    setvl(value);
  }

  kotoage fn run() permission(Admin) {
    helper(8);
  }
}
`);
  const invalidSetvlRange = compileKotodamaProgram(`
seiyaku InvalidSetvlRange {
  fn helper() {
    setvl(256);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidAlias = compileKotodamaProgram(`
seiyaku InvalidAlias {
  fn helper() {
    let owner = resolve_account_alias(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidTlvLen = compileKotodamaProgram(`
seiyaku InvalidTlvLen {
  fn helper() {
    let size = tlv_len(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidPointerToNorito = compileKotodamaProgram(`
seiyaku InvalidPointerToNorito {
  fn helper() {
    let bytes = pointer_to_norito(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidPointerToNoritoJson = compileKotodamaProgram(`
seiyaku InvalidPointerToNoritoJson {
  fn helper() {
    let bytes = pointer_to_norito(json!{ ok: true });
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidEncodeSchema = compileKotodamaProgram(`
seiyaku InvalidEncodeSchema {
  fn helper() {
    let bytes = encode_schema(name("example.schema"), 1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidDecodeSchema = compileKotodamaProgram(`
seiyaku InvalidDecodeSchema {
  fn helper() {
    let payload = decode_schema(1, norito_bytes("0x0102"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidSchemaInfo = compileKotodamaProgram(`
seiyaku InvalidSchemaInfo {
  fn helper() {
    let info_json = schema_info(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidHash = compileKotodamaProgram(`
seiyaku InvalidHash {
  fn helper() {
    let digest = sha256_hash(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidEncodeDecode = compileKotodamaProgram(`
seiyaku InvalidEncodeDecode {
  fn helper() {
    let bytes = encode_json(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidDecodeInt = compileKotodamaProgram(`
seiyaku InvalidDecodeInt {
  fn helper() {
    let value = decode_int(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidVrf = compileKotodamaProgram(`
seiyaku InvalidVrf {
  fn helper() {
    let payload = blob("0x010203");
    let proof = vrf_verify(payload, payload, payload, name("variant"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidVrfBatch = compileKotodamaProgram(`
seiyaku InvalidVrfBatch {
  fn helper() {
    let proof = vrf_verify_batch(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidSm2Arity = compileKotodamaProgram(`
seiyaku InvalidSm2Arity {
  fn helper() {
    let payload = blob("0x010203");
    let ok = sm2_verify(payload, payload);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidSm2Distid = compileKotodamaProgram(`
seiyaku InvalidSm2Distid {
  fn helper() {
    let payload = blob("0x010203");
    let ok = sm2_verify(payload, payload, payload, 1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidSignatureScheme = compileKotodamaProgram(`
seiyaku InvalidSignatureScheme {
  fn helper() {
    let payload = blob("0x010203");
    let ok = verify_signature(payload, payload, payload, name("scheme"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidSm4Gcm = compileKotodamaProgram(`
seiyaku InvalidSm4Gcm {
  fn helper() {
    let payload = blob("0x010203");
    let sealed = sm4_gcm_seal(payload, payload, payload, 1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidSm4CcmTag = compileKotodamaProgram(`
seiyaku InvalidSm4CcmTag {
  fn helper() {
    let payload = blob("0x010203");
    let sealed = sm4_ccm_seal(payload, payload, payload, payload, name("tag"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const validHostHelpers = compileKotodamaProgram(`
seiyaku ValidHostHelpers {
  fn helper(payload: Blob) {
    execute_instruction(payload);
    zk_verify_unshield(payload);
    set_execution_depth(1);
    let alias = norito_bytes("banking@centralbank");
    let owner = resolve_account_alias(alias);
    let schema = name("example.schema");
    let encoded = encode_schema(schema, json!{ owner: "alice" });
    let decoded = decode_schema(schema, encoded);
    let info_json = schema_info(schema);
    let digest = sha3_hash(payload);
    let int_bytes = encode_int(7);
    let decoded_int = decode_int(int_bytes);
    let json_bytes = encode_json(decoded);
    let decoded_json = decode_json(json_bytes);
    let vrf = vrf_verify(payload, payload, payload, 1);
    let vrf_batch = vrf_verify_batch(payload);
    let sm2 = sm2_verify(payload, payload, payload);
    let sm2_distid = sm2_verify(payload, payload, payload, payload);
    let signature = verify_signature(payload, payload, payload, 0);
    let owner_bytes = pointer_to_norito(owner);
    let owner_len = tlv_len(owner_bytes);
    let decoded_len = tlv_len(decoded);
    let gcm = sm4_gcm_seal(payload, payload, payload, payload);
    let opened_gcm = sm4_gcm_open(payload, payload, payload, gcm);
    let ccm = sm::seal_ccm(payload, payload, payload, payload, 12);
    let opened_ccm = sm::open_ccm(payload, payload, payload, ccm);
    assert(sm2, "sm2");
    assert(sm2_distid, "sm2_distid");
    assert(signature, "signature");
    info(tlv_len(digest));
    info(decoded_int);
    info(tlv_len(json_bytes));
    info(tlv_len(owner_bytes));
    info(owner_len);
    info(decoded_len);
    info(tlv_len(opened_gcm));
    info(tlv_len(opened_ccm));
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(invalidExecute.artifactBytes.length, 0);
  assert.equal(invalidExecute.diagnostics.length, 1);
  assert.match(
    invalidExecute.diagnostics[0].message,
    /execute_instruction expects \(Blob\|bytes\) where the argument is a pointer to NoritoBytes TLV in INPUT/,
  );
  assert.equal(invalidZk.artifactBytes.length, 0);
  assert.equal(invalidZk.diagnostics.length, 1);
  assert.match(
    invalidZk.diagnostics[0].message,
    /zk_verify_unshield expects \(Blob\|bytes\) where the argument is a pointer to NoritoBytes TLV in INPUT/,
  );
  assert.equal(invalidStateGet.artifactBytes.length, 0);
  assert.equal(invalidStateGet.diagnostics.length, 1);
  assert.match(invalidStateGet.diagnostics[0].message, /state_get expects \(Name\)/);
  assert.equal(invalidStateSet.artifactBytes.length, 0);
  assert.equal(invalidStateSet.diagnostics.length, 1);
  assert.match(invalidStateSet.diagnostics[0].message, /state_set expects \(Name, Blob\|bytes\)/);
  assert.equal(invalidStateDel.artifactBytes.length, 0);
  assert.equal(invalidStateDel.diagnostics.length, 1);
  assert.match(invalidStateDel.diagnostics[0].message, /state_del expects \(Name\)/);
  assert.equal(invalidDepth.artifactBytes.length, 0);
  assert.equal(invalidDepth.diagnostics.length, 1);
  assert.match(invalidDepth.diagnostics[0].message, /set_execution_depth expects one int arg/);
  assert.equal(invalidSetvlDynamic.artifactBytes.length, 0);
  assert.equal(invalidSetvlDynamic.diagnostics.length, 1);
  assert.match(invalidSetvlDynamic.diagnostics[0].message, /setvl expects a literal int in range 0..=255/);
  assert.equal(invalidSetvlRange.artifactBytes.length, 0);
  assert.equal(invalidSetvlRange.diagnostics.length, 1);
  assert.match(invalidSetvlRange.diagnostics[0].message, /setvl value must be in range 0..=255, got 256/);
  assert.equal(invalidAlias.artifactBytes.length, 0);
  assert.equal(invalidAlias.diagnostics.length, 1);
  assert.match(invalidAlias.diagnostics[0].message, /resolve_account_alias expects \(String\|Blob\)/);
  assert.equal(invalidTlvLen.artifactBytes.length, 0);
  assert.equal(invalidTlvLen.diagnostics.length, 1);
  assert.match(
    invalidTlvLen.diagnostics[0].message,
    /tlv_len expects a pointer-ABI type, Json, or Blob\|bytes argument/,
  );
  assert.equal(invalidPointerToNorito.artifactBytes.length, 0);
  assert.equal(invalidPointerToNorito.diagnostics.length, 1);
  assert.match(
    invalidPointerToNorito.diagnostics[0].message,
    /pointer_to_norito expects a pointer-ABI type or Blob\|bytes argument/,
  );
  assert.equal(invalidPointerToNoritoJson.artifactBytes.length, 0);
  assert.equal(invalidPointerToNoritoJson.diagnostics.length, 1);
  assert.match(
    invalidPointerToNoritoJson.diagnostics[0].message,
    /pointer_to_norito expects a pointer-ABI type or Blob\|bytes argument/,
  );
  assert.equal(invalidEncodeSchema.artifactBytes.length, 0);
  assert.equal(invalidEncodeSchema.diagnostics.length, 1);
  assert.match(invalidEncodeSchema.diagnostics[0].message, /encode_schema expects \(Name, Json\)/);
  assert.equal(invalidDecodeSchema.artifactBytes.length, 0);
  assert.equal(invalidDecodeSchema.diagnostics.length, 1);
  assert.match(invalidDecodeSchema.diagnostics[0].message, /decode_schema expects \(Name, Blob\|bytes\)/);
  assert.equal(invalidSchemaInfo.artifactBytes.length, 0);
  assert.equal(invalidSchemaInfo.diagnostics.length, 1);
  assert.match(invalidSchemaInfo.diagnostics[0].message, /schema_info expects \(Name\)/);
  assert.equal(invalidHash.artifactBytes.length, 0);
  assert.equal(invalidHash.diagnostics.length, 1);
  assert.match(
    invalidHash.diagnostics[0].message,
    /sha256_hash expects \(Blob\|bytes\) argument pointing to INPUT TLV/,
  );
  assert.equal(invalidEncodeDecode.artifactBytes.length, 0);
  assert.equal(invalidEncodeDecode.diagnostics.length, 1);
  assert.match(invalidEncodeDecode.diagnostics[0].message, /encode_json expects \(Json\)/);
  assert.equal(invalidDecodeInt.artifactBytes.length, 0);
  assert.equal(invalidDecodeInt.diagnostics.length, 1);
  assert.match(invalidDecodeInt.diagnostics[0].message, /decode_int expects \(Blob\|bytes\)/);
  assert.equal(invalidVrf.artifactBytes.length, 0);
  assert.equal(invalidVrf.diagnostics.length, 1);
  assert.match(
    invalidVrf.diagnostics[0].message,
    /vrf_verify expects \(Blob\|bytes, Blob\|bytes, Blob\|bytes, int variant\)/,
  );
  assert.equal(invalidVrfBatch.artifactBytes.length, 0);
  assert.equal(invalidVrfBatch.diagnostics.length, 1);
  assert.match(invalidVrfBatch.diagnostics[0].message, /vrf_verify_batch expects \(Blob\|bytes\)/);
  assert.equal(invalidSm2Arity.artifactBytes.length, 0);
  assert.equal(invalidSm2Arity.diagnostics.length, 1);
  assert.match(
    invalidSm2Arity.diagnostics[0].message,
    /sm2_verify expects \(Blob, Blob, Blob\) or \(Blob, Blob, Blob, Blob\) where arguments reference INPUT TLVs/,
  );
  assert.equal(invalidSm2Distid.artifactBytes.length, 0);
  assert.equal(invalidSm2Distid.diagnostics.length, 1);
  assert.match(
    invalidSm2Distid.diagnostics[0].message,
    /sm2_verify optional distid must be provided as Blob\|bytes pointer/,
  );
  assert.equal(invalidSignatureScheme.artifactBytes.length, 0);
  assert.equal(invalidSignatureScheme.diagnostics.length, 1);
  assert.match(
    invalidSignatureScheme.diagnostics[0].message,
    /verify_signature expects scheme code as int/,
  );
  assert.equal(invalidSm4Gcm.artifactBytes.length, 0);
  assert.equal(invalidSm4Gcm.diagnostics.length, 1);
  assert.match(
    invalidSm4Gcm.diagnostics[0].message,
    /sm4_gcm_seal expects \(Blob\|bytes, Blob\|bytes, Blob\|bytes, Blob\|bytes\)/,
  );
  assert.equal(invalidSm4CcmTag.artifactBytes.length, 0);
  assert.equal(invalidSm4CcmTag.diagnostics.length, 1);
  assert.match(
    invalidSm4CcmTag.diagnostics[0].message,
    /sm4_ccm_seal optional tag length must be int/,
  );
  assert.deepEqual(validHostHelpers.diagnostics, []);
  const validHostCode = readArtifactCode(validHostHelpers.artifactBytes);
  assert.notEqual(validHostCode.indexOf(syscallNeedle(0x5d)), -1, "missing POINTER_TO_NORITO syscall");
  assert.notEqual(validHostCode.indexOf(syscallNeedle(0x77)), -1, "missing TLV_LEN syscall");
});

test("Kotodama compiler SDK rejects invalid execute_query payloads semantically", () => {
  const invalidPayload = compileKotodamaProgram(`
seiyaku InvalidExecuteQueryPayload {
  fn helper() {
    let result = execute_query(norito_bytes(1));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const validPayloads = compileKotodamaProgram(`
seiyaku ValidExecuteQueryPayloads {
  fn helper() {
    let raw = blob("0x00");
    let from_blob = execute_query(norito_bytes(raw));
    let from_literal = execute_query(norito_bytes("0x00"));
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(invalidPayload.artifactBytes.length, 0);
  assert.equal(invalidPayload.diagnostics.length, 1);
  assert.match(invalidPayload.diagnostics[0].message, /execute_query expects norito_bytes\(string or Blob\|bytes\)/);
  assert.deepEqual(validPayloads.diagnostics, []);
});

test("Kotodama compiler SDK types execute_query results as bytes", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ExecuteQueryResultBytes {
  fn helper() {
    let result = execute_query(norito_bytes("0x00"));
    let amount = result.get_int(name("amount"));
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(compiled.diagnostics[0].message, /get_int expects \(Json, Name\)/);
});

test("Kotodama compiler SDK compiles expression-form execute_query", () => {
  const valid = compileKotodamaProgram(`
seiyaku ExecuteQueryExpression {
  fn helper() -> Blob {
    return execute_query(norito_bytes("0x00"));
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalid = compileKotodamaProgram(`
seiyaku InvalidExecuteQueryExpression {
  fn helper() -> Blob {
    return execute_query(1);
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const code = readArtifactCode(valid.artifactBytes);

  assert.deepEqual(valid.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0xa1)), -1, "missing SMARTCONTRACT_EXECUTE_QUERY syscall");
  assert.equal(invalid.artifactBytes.length, 0);
  assert.equal(invalid.diagnostics.length, 1);
  assert.match(
    invalid.diagnostics[0].message,
    /execute_query expects \(Blob\|bytes\) where the argument is a pointer to NoritoBytes TLV in INPUT/,
  );
});

test("Kotodama compiler SDK rejects test-only helpers outside test functions", () => {
  const cases = [
    {
      name: "invoke_entrypoint",
      statement: 'let out = invoke_entrypoint("run", json("{}"));',
    },
    {
      name: "invoke_entrypoint_as",
      statement: 'let out = invoke_entrypoint_as("issuer", "run", json("{}"));',
    },
    {
      name: "expect_reject_as",
      statement: 'expect_reject_as("issuer", "run", json("{}"));',
    },
    {
      name: "actor_account",
      statement: 'let acct = actor_account("issuer");',
    },
    {
      name: "actor_public_key",
      statement: 'let pk = actor_public_key("issuer");',
    },
    {
      name: "actor_sign",
      statement: 'let sig = actor_sign("issuer", blob("0x00"));',
    },
  ];

  for (const { name, statement } of cases) {
    const compiled = compileKotodamaProgram(`
seiyaku TestOnlyHelper {
  fn helper() {
    ${statement}
  }

  kotoage fn run() permission(Admin) {}
}
`);

    assert.equal(compiled.artifactBytes.length, 0, name);
    assert.equal(compiled.diagnostics.length, 1, name);
    assert.match(
      compiled.diagnostics[0].message,
      new RegExp(`\`${name}\` is only available inside #\\[test\\] Kotodama functions`),
      name,
    );
  }
});

test("Kotodama compiler SDK strips Rust test functions in production mode", () => {
  const compiled = compileKotodamaProgram(`
seiyaku StrippedTests {
  kotoage fn run() permission(Admin) {
    info("run");
  }

  #[test(fixture="seeded")]
  fn smoke() {
    let out = invoke_entrypoint("run", json("{}"));
    let acct = actor_account("issuer");
    let pk = actor_public_key("issuer");
    let sig = actor_sign("issuer", b"demo");
    expect_reject_as("issuer", "run", json("{\\"count\\":-1}"));
  }

  #[テスト]
  fn unicode_smoke() {
    let out = invoke_entrypoint("run", json("{}"));
  }
}
`);
  const invalidOption = compileKotodamaProgram(`
seiyaku InvalidTestAttribute {
  kotoage fn run() permission(Admin) {}

  #[test(seed="demo")]
  fn smoke() {}
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(compiled.manifest?.entrypoints.length, 1);
  assert.equal(compiled.manifest?.entrypoints[0]?.name, "run");
  assert.equal(compiled.manifest?.entrypoints.some((entrypoint) => entrypoint.name === "smoke"), false);
  assert.equal(invalidOption.artifactBytes.length, 0);
  assert.equal(invalidOption.diagnostics.length, 1);
  assert.match(invalidOption.diagnostics[0].message, /unknown test attribute option `seed`/);
});

test("Kotodama compiler SDK emits private test helper syscalls in test mode", () => {
  const compiled = compileKotodamaProgram(`
seiyaku TestModeHelpers {
  kotoage fn run(count: int) -> int {
    return count + 1;
  }

  #[test]
  fn smoke() {
    let next = invoke_entrypoint_as("issuer", "run", json("{\\"count\\":7}"));
    let acct = actor_account("issuer");
    let pk = actor_public_key(name("issuer"));
    let sig = actor_sign("issuer", b"message");
    expect_reject_as("issuer", "run", json("{\\"count\\":-1}"));
    let held = (next, acct, pk, sig);
  }
}
`, { mode: "test" });
  const production = compileKotodamaProgram(`
seiyaku TestModeHelpers {
  kotoage fn run(count: int) -> int {
    return count + 1;
  }

  #[test]
  fn smoke() {
    let next = invoke_entrypoint_as("issuer", "run", json("{\\"count\\":7}"));
    let acct = actor_account("issuer");
    let pk = actor_public_key("issuer");
    let sig = actor_sign("issuer", b"message");
    expect_reject_as("issuer", "run", json("{\\"count\\":-1}"));
    let held = (next, acct, pk, sig);
  }
}
`);

  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallxNeedle(0x00fe_0001)), -1);
  assert.notEqual(code.indexOf(syscallxNeedle(0x00fe_0002)), -1);
  assert.notEqual(code.indexOf(syscallxNeedle(0x00fe_0003)), -1);
  assert.notEqual(code.indexOf(syscallxNeedle(0x00fe_0004)), -1);
  assert.notEqual(code.indexOf(syscallxNeedle(0x00fe_0005)), -1);
  assert.ok(compiled.sourceMap.some((entry) => entry.function_name === "smoke"));
  assert.ok(production.sourceMap.every((entry) => entry.function_name !== "smoke"));
});

test("Kotodama compiler SDK mirrors Rust invoke_entrypoint_as return-pointer flags", () => {
  const amount = compileKotodamaProgram(`
seiyaku InvokeAsAmountFlag {
  kotoage fn run() -> Amount {
    let amount: Amount = 7;
    return amount;
  }

  #[test]
  fn smoke() {
    let held = invoke_entrypoint_as("issuer", "run", json("{}"));
  }
}
`, { mode: "test" });
  const json = compileKotodamaProgram(`
seiyaku InvokeAsJsonFlag {
  kotoage fn run() -> Json {
    return json("{}");
  }

  #[test]
  fn smoke() {
    let held = invoke_entrypoint_as("issuer", "run", json("{}"));
  }
}
`, { mode: "test" });
  const name = compileKotodamaProgram(`
seiyaku InvokeAsNameFlag {
  kotoage fn run() -> Name {
    return name("ok");
  }

  #[test]
  fn smoke() {
    let held = invoke_entrypoint_as("issuer", "run", json("{}"));
  }
}
`, { mode: "test" });

  assert.deepEqual(amount.diagnostics, []);
  assert.deepEqual(json.diagnostics, []);
  assert.deepEqual(name.diagnostics, []);
  assert.notEqual(
    readArtifactCode(amount.artifactBytes).indexOf(invokeEntrypointAsFlagNeedle(0)),
    -1,
    "Amount is a wide numeric ABI value, not a Rust pointer return",
  );
  assert.notEqual(
    readArtifactCode(json.artifactBytes).indexOf(invokeEntrypointAsFlagNeedle(1)),
    -1,
    "Json return should set the Rust pointer flag",
  );
  assert.notEqual(
    readArtifactCode(name.artifactBytes).indexOf(invokeEntrypointAsFlagNeedle(1)),
    -1,
    "Name return should set the Rust pointer flag",
  );
});

test("Kotodama compiler SDK keeps test helpers scoped to test functions in test mode", () => {
  const nonTestHelper = compileKotodamaProgram(`
seiyaku TestModeScope {
  kotoage fn run() {
    let acct = actor_account("issuer");
  }
}
`, { mode: "test" });
  const dynamicTarget = compileKotodamaProgram(`
seiyaku DynamicDirectInvoke {
  kotoage fn run() {}

  #[test]
  fn smoke() {
    let target = "run";
    invoke_entrypoint(target, json("{}"));
  }
}
`, { mode: "test" });

  assert.equal(nonTestHelper.artifactBytes.length, 0);
  assert.match(
    nonTestHelper.diagnostics[0]?.message ?? "",
    /`actor_account` is only available inside #\[test\] Kotodama functions/,
  );
  assert.equal(dynamicTarget.artifactBytes.length, 0);
  assert.match(dynamicTarget.diagnostics[0]?.message ?? "", /invoke_entrypoint requires a literal entrypoint name/);
});

test("Kotodama compiler SDK lowers direct invoke_entrypoint through test payload override", () => {
  const compiled = compileKotodamaProgram(`
seiyaku DirectInvoke {
  kotoage fn run(count: int) -> int {
    return count + 1;
  }

  #[test]
  fn smoke() {
    let next = invoke_entrypoint("run", json("{\\"count\\":7}"));
    assert_eq(next, 8);
  }
}
`, { mode: "test" });
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(code.indexOf(syscallxNeedle(0x00fe_0004)), -1, "direct invoke should not use the actor-host private syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x50)), -1, "missing STATE_GET override read");
  assert.notEqual(code.indexOf(syscallNeedle(0x51)), -1, "missing STATE_SET override write");
  assert.notEqual(code.indexOf(syscallNeedle(0x52)), -1, "missing STATE_DEL override cleanup");
  assert.notEqual(code.indexOf(syscallNeedle(0x57)), -1, "missing JSON_ENCODE override payload");
  assert.notEqual(code.indexOf(syscallNeedle(0x58)), -1, "missing JSON_DECODE wrapper override payload");
  assert.ok(compiled.sourceMap.some((entry) => entry.function_name === "smoke"));
});

test("Kotodama compiler SDK lowers direct invoke_entrypoint tuple returns through wrapper multi-return", () => {
  const compiled = compileKotodamaProgram(`
seiyaku DirectTupleInvoke {
  kotoage fn run(count: int) -> (int, int) {
    return (count, count + 1);
  }

  #[test]
  fn smoke() {
    let pair = invoke_entrypoint("run", json("{\\"count\\":7}"));
    assert_eq(pair.0, 7);
    assert_eq(pair.1, 8);
  }
}
`, { mode: "test" });
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(code.indexOf(syscallxNeedle(0x00fe_0004)), -1, "direct invoke should not use the actor-host private syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0x50)), -1, "missing STATE_GET override read");
  assert.notEqual(code.indexOf(syscallNeedle(0x51)), -1, "missing STATE_SET override write");
  assert.notEqual(code.indexOf(syscallNeedle(0x52)), -1, "missing STATE_DEL override cleanup");
  assert.notEqual(code.indexOf(syscallNeedle(0x57)), -1, "missing JSON_ENCODE override payload");
  assert.notEqual(code.indexOf(syscallNeedle(0x58)), -1, "missing JSON_DECODE wrapper override payload");
  assert.ok(compiled.sourceMap.some((entry) => entry.function_name === "smoke"));
});

test("Kotodama compiler SDK ignores Rust fixture declarations in production mode", () => {
  const compiled = compileKotodamaProgram(`
koto_test { target: "contracts/demo.ko" }

fixture seeded {
  caller(account!("alice@wonderland"));
  grant_permission("register_domain");
}

seiyaku FixtureProduction {
  fixture local {
    caller(account!("bob@wonderland"));
    note(json!{ ok: true, nested: [1, 2] });
  }

  kotoage fn run() permission(Admin) {
    info("run");
  }

  #[test(fixture="local")]
  fn smoke() {
    let out = invoke_entrypoint("run", json("{}"));
  }
}

fixture cleanup {
  caller(account!("carol@wonderland"));
}
`);
  const invalidTarget = compileKotodamaProgram(`
koto_test { source: "contracts/demo.ko" }

seiyaku InvalidKotoTest {
  kotoage fn run() permission(Admin) {}
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(compiled.manifest?.entrypoints.length, 1);
  assert.equal(compiled.manifest?.entrypoints[0]?.name, "run");
  assert.equal(compiled.manifest?.entrypoints.some((entrypoint) => entrypoint.name === "smoke"), false);
  assert.equal(invalidTarget.artifactBytes.length, 0);
  assert.equal(invalidTarget.diagnostics.length, 1);
  assert.equal(invalidTarget.diagnostics[0].message, "parser error: {error}: expected target but found Colon");
});

test("Kotodama compiler SDK mirrors Rust auxiliary declaration parser diagnostics", () => {
  const invalidKotoTestKey = compileKotodamaProgram(`
koto_test { source: "contracts/demo.ko" }
fn run() {}
`);
  const invalidKotoTestTarget = compileKotodamaProgram(`
koto_test { target: true }
fn run() {}
`);
  const invalidFixtureName = compileKotodamaProgram(`
fixture call {}
fn run() {}
`);
  const invalidFixtureAction = compileKotodamaProgram(`
fixture seeded {
  call }
fn run() {}
`);
  const invalidFixtureArgs = compileKotodamaProgram(`
fixture seeded {
  action(1 2)
}
fn run() {}
`);
  const invalidKotobaKey = compileKotodamaProgram(`
kotoba { true: { en: "Hello" } }
fn run() {}
`);
  const invalidKotobaText = compileKotodamaProgram(`
kotoba { hello: { en: 1 } }
fn run() {}
`);

  assert.equal(invalidKotoTestKey.artifactBytes.length, 0);
  assert.equal(invalidKotoTestKey.diagnostics[0].message, "parser error: {error}: expected target but found Colon");
  assert.equal(invalidKotoTestTarget.artifactBytes.length, 0);
  assert.equal(invalidKotoTestTarget.diagnostics[0].message, "parser error: {error}: expected identifier or string literal but found True");
  assert.equal(invalidFixtureName.artifactBytes.length, 0);
  assert.equal(invalidFixtureName.diagnostics[0].message, "parser error: {error}: expected identifier but found Call");
  assert.equal(invalidFixtureAction.artifactBytes.length, 0);
  assert.equal(invalidFixtureAction.diagnostics[0].message, "parser error: expected identifier but found Call");
  assert.equal(invalidFixtureArgs.artifactBytes.length, 0);
  assert.equal(invalidFixtureArgs.diagnostics[0].message, "parser error: expected RParen but found Number(2)");
  assert.equal(invalidKotobaKey.artifactBytes.length, 0);
  assert.equal(invalidKotobaKey.diagnostics[0].message, "parser error: {error}: expected identifier or string literal but found True");
  assert.equal(invalidKotobaText.artifactBytes.length, 0);
  assert.equal(invalidKotobaText.diagnostics[0].message, "parser error: {error}: expected string literal but found Number(1)");
});

test("Kotodama compiler SDK parses Rust byte string literals", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ByteStringLiterals {
  kotoage fn run() permission(Admin) {
    let escaped = norito_bytes(b"ab\\x41");
    let raw = br"ab\\n";
    let raw_hash = norito_bytes(rb#"a "quote""#);
    execute_instruction(escaped);
    execute_instruction(raw_hash);
    let result = execute_query(norito_bytes(raw));
    info(1);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);
  const entries = readLiteralTableEntries(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0xa0)), -1, "missing SMARTCONTRACT_EXECUTE_INSTRUCTION syscall");
  assert.notEqual(code.indexOf(syscallNeedle(0xa1)), -1, "missing SMARTCONTRACT_EXECUTE_QUERY syscall");
  assert.ok(entries.some((entry) => entry.type === 9 && entry.payload.equals(Buffer.from("abA"))));
  assert.ok(entries.some((entry) => entry.type === 6 && entry.payload.equals(Buffer.from("ab\\n"))));
  assert.ok(entries.some((entry) => entry.type === 9 && entry.payload.equals(Buffer.from('a "quote"'))));
});

test("Kotodama compiler SDK lowers upstream sc_execute instruction aliases", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ScExecuteAliases {
  kotoage fn run() permission(Admin) {
    sc_execute_submit_ballot(norito_bytes("0x0102"));
    sc_execute_unshield(norito_bytes("0x0304"));
  }
}
`);
  const view = compileKotodamaProgram(`
seiyaku ViewScExecuteAlias {
  view fn inspect() -> int {
    sc_execute_unshield(norito_bytes("0x0102"));
    return 1;
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);
  const executeInstructionNeedle = syscallNeedle(0xa0);
  const firstInstructionCall = code.indexOf(executeInstructionNeedle);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(firstInstructionCall, -1, "missing first SMARTCONTRACT_EXECUTE_INSTRUCTION syscall");
  assert.notEqual(code.indexOf(executeInstructionNeedle, firstInstructionCall + 1), -1, "missing second SMARTCONTRACT_EXECUTE_INSTRUCTION syscall");
  assert.equal(view.artifactBytes.length, 0);
  assert.equal(view.diagnostics.length, 1);
  assert.match(view.diagnostics[0].message, /view function `inspect` cannot perform instruction emission/);
});

test("Kotodama compiler SDK mirrors Rust lexer diagnostics for string and byte escapes", () => {
  const cases = [
    [
      'string unknown escape',
      'fn main() { let value = "\\q"; }',
      'parser error: unknown escape \\q at 1:25',
    ],
    [
      'byte unknown escape',
      'fn main() { let value = b"\\q"; }',
      'parser error: unknown escape \\q at 1:25',
    ],
    [
      'string invalid hex',
      'fn main() { let value = "\\xG1"; }',
      "parser error: invalid hex digit 'G' in escape at 1:25",
    ],
    [
      'byte invalid hex',
      'fn main() { let value = b"\\xG1"; }',
      "parser error: invalid hex digit 'G' in escape at 1:25",
    ],
    [
      'string empty unicode',
      'fn main() { let value = "\\u{}"; }',
      'parser error: empty unicode escape at 1:25',
    ],
    [
      'byte empty unicode',
      'fn main() { let value = b"\\u{}"; }',
      'parser error: empty unicode escape at 1:25',
    ],
    [
      'string unicode missing opener',
      'fn main() { let value = "\\u1234"; }',
      "parser error: {error}: unicode escape at 1:25 must start with '{'",
    ],
    [
      'byte unicode missing opener',
      'fn main() { let value = b"\\u1234"; }',
      "parser error: {error}: unicode escape at 1:25 must start with '{'",
    ],
    [
      'string newline',
      'fn main() { let value = "abc\n"; }',
      'parser error: unterminated string literal at 1:25: newline before closing quote',
    ],
    [
      'byte missing quote',
      'fn main() { let value = b"abc; }',
      'parser error: unterminated byte string literal at 1:25: missing closing quote',
    ],
    [
      'unexpected character',
      'fn main() { let value = @; }',
      "parser error: Unexpected character '@' at 1:25",
    ],
  ];

  for (const [name, source, message] of cases) {
    const compiled = compileKotodamaProgram(source);
    assert.equal(compiled.artifactBytes.length, 0, name);
    assert.equal(compiled.diagnostics.length, 1, name);
    assert.equal(compiled.diagnostics[0].message, message, name);
  }
});

test("Kotodama compiler SDK parses Rust raw string literals", () => {
  const compiled = compileKotodamaProgram(`
seiyaku RawStringLiterals {
  kotoage fn run() permission(Admin) {
    execute_instruction(norito_bytes(r"ab\\n"));
    execute_instruction(norito_bytes(r#"a "quote""#));
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);
  const entries = readLiteralTableEntries(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0xa0)), -1, "missing SMARTCONTRACT_EXECUTE_INSTRUCTION syscall");
  assert.ok(entries.some((entry) => entry.type === 9 && entry.payload.equals(Buffer.from("ab\\n"))));
  assert.ok(entries.some((entry) => entry.type === 9 && entry.payload.equals(Buffer.from('a "quote"'))));
});

test("Kotodama compiler SDK rejects unterminated raw string literals", () => {
  const cases = [
    [
      'raw missing quote',
      'fn main() { let value = r#abc; }',
      'parser error: expected \'"\' after raw string prefix at 1:25',
    ],
    [
      'raw unterminated',
      'fn main() { let value = r#"abc; }',
      'parser error: unterminated raw string literal at 1:25: missing closing delimiter',
    ],
    [
      'raw hash unterminated',
      'fn main() { let value = r##"abc"#; }',
      'parser error: unterminated raw string literal at 1:25: missing closing delimiter',
    ],
    [
      'byte raw missing quote',
      'fn main() { let value = br#abc; }',
      'parser error: expected \'"\' after raw string prefix at 1:25',
    ],
    [
      'byte raw unterminated',
      'fn main() { let value = br#"abc; }',
      'parser error: unterminated raw string literal at 1:25: missing closing delimiter',
    ],
    [
      'reverse byte raw missing quote',
      'fn main() { let value = rb#abc; }',
      'parser error: expected \'"\' after raw string prefix at 1:25',
    ],
    [
      'reverse byte raw unterminated',
      'fn main() { let value = rb#"abc; }',
      'parser error: unterminated raw string literal at 1:25: missing closing delimiter',
    ],
  ];

  for (const [name, source, message] of cases) {
    const compiled = compileKotodamaProgram(source);
    assert.equal(compiled.artifactBytes.length, 0, name);
    assert.equal(compiled.diagnostics.length, 1, name);
    assert.equal(compiled.diagnostics[0].message, message, name);
  }
});

test("Kotodama compiler SDK parses Rust hex and binary integer literals", () => {
  const compiled = compileKotodamaProgram(`
seiyaku RadixIntegerLiterals {
  kotoage fn run() permission(Admin) {
    let hex = 0x2a;
    let binary = 0b1010_0000;
    info(hex + binary);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(readArtifactCode(compiled.artifactBytes).length, 0);
});

test("Kotodama compiler SDK rejects radix integer literals without digits", () => {
  const hexadecimal = compileKotodamaProgram(`
seiyaku InvalidHexIntegerLiteral {
  kotoage fn run() permission(Admin) {
    let value = 0x_;
    info(value);
  }
}
`);
  const binary = compileKotodamaProgram(`
seiyaku InvalidBinaryIntegerLiteral {
  kotoage fn run() permission(Admin) {
    let value = 0b_;
    info(value);
  }
}
`);

  assert.equal(hexadecimal.artifactBytes.length, 0);
  assert.equal(hexadecimal.diagnostics.length, 1);
  assert.equal(hexadecimal.diagnostics[0].message, "parser error: {error}: expected hexadecimal digits after 0x");
  assert.equal(binary.artifactBytes.length, 0);
  assert.equal(binary.diagnostics.length, 1);
  assert.equal(binary.diagnostics[0].message, "parser error: {error}: expected binary digits after 0b");
});

test("Kotodama compiler SDK parses Rust i64 integer literal suffixes", () => {
  const compiled = compileKotodamaProgram(`
seiyaku SuffixedIntegerLiterals {
  kotoage fn run() permission(Admin) {
    let decimal = 1i64;
    let hex = 0x2ai64;
    let binary = 0b10i64;
    info(decimal + hex + binary);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(readArtifactCode(compiled.artifactBytes).length, 0);
});

test("Kotodama compiler SDK mirrors Rust signed i64 integer literal range", () => {
  const valid = compileKotodamaProgram(`
seiyaku I64LiteralBounds {
  kotoage fn run() permission(Admin) {
    let max = 9223372036854775807;
    let min = -9223372036854775808;
    info(1);
  }
}
`);
  const positiveOverflow = compileKotodamaProgram(`
seiyaku I64PositiveOverflow {
  kotoage fn run() permission(Admin) {
    let value = 9223372036854775808;
    info(1);
  }
}
`);
  const negativeOverflow = compileKotodamaProgram(`
seiyaku I64NegativeOverflow {
  kotoage fn run() permission(Admin) {
    let value = -9223372036854775809;
    info(1);
  }
}
`);

  assert.deepEqual(valid.diagnostics, []);
  assert.notEqual(valid.artifactBytes.length, 0);
  assert.equal(positiveOverflow.artifactBytes.length, 0);
  assert.equal(positiveOverflow.diagnostics[0].message, "parser error: {error}: integer literal out of range (max 9223372036854775807)");
  assert.equal(negativeOverflow.artifactBytes.length, 0);
  assert.equal(negativeOverflow.diagnostics[0].message, "parser error: {error}: integer literal out of range (min -9223372036854775808)");
});

test("Kotodama compiler SDK rejects unknown integer literal suffixes", () => {
  const compiled = compileKotodamaProgram(`
seiyaku InvalidIntegerLiteralSuffix {
  kotoage fn run() permission(Admin) {
    let value = 1i128;
    info(value);
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(compiled.diagnostics[0].message, /unknown integer literal suffix `i128`/);
});

test("Kotodama compiler SDK rejects fractional decimal literals with Rust-shaped diagnostics", () => {
  const compiled = compileKotodamaProgram(`
seiyaku InvalidDecimalLiteral {
  kotoage fn run() permission(Admin) {
    let value = 1.25;
    info(value);
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(compiled.diagnostics[0].message, /numeric literal `1.25` must be an unsigned integer \(scale=0\)/);
});

test("Kotodama compiler SDK rejects all-zero fractional decimal literals", () => {
  const compiled = compileKotodamaProgram(`
seiyaku InvalidZeroFractionDecimal {
  kotoage fn run() permission(Admin) {
    let value = 1.000;
    info(value);
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(compiled.diagnostics[0].message, /numeric literal `1.000` must be an unsigned integer \(scale=0\)/);
});

test("Kotodama compiler SDK rejects decimal literals without fractional digits", () => {
  const missingField = compileKotodamaProgram(`
seiyaku MissingDecimalFraction {
  kotoage fn run() permission(Admin) {
    let value = 1.;
    info(value);
  }
}
`);
  const underscoreField = compileKotodamaProgram(`
seiyaku DecimalDotUnderscoreField {
  kotoage fn run() permission(Admin) {
    let value = 1._;
    info(value);
  }
}
`);
  const underscoreDigitField = compileKotodamaProgram(`
seiyaku DecimalDotUnderscoreDigitField {
  kotoage fn run() permission(Admin) {
    let value = 1.__2;
    info(value);
  }
}
`);
  const doubleDot = compileKotodamaProgram(`
seiyaku DecimalDoubleDot {
  kotoage fn run() permission(Admin) {
    let value = 1..2;
    info(value);
  }
}
`);
  const rightParen = compileKotodamaProgram(`
seiyaku DecimalDotRightParen {
  kotoage fn run() permission(Admin) {
    info((1.));
  }
}
`);
  const oneLineMissingField = compileKotodamaProgram("fn main() { let value = 1.; }");

  assert.equal(missingField.artifactBytes.length, 0);
  assert.equal(missingField.diagnostics.length, 1);
  assert.equal(missingField.diagnostics[0].message, "parser error: expected identifier or tuple index but found Semicolon");
  assert.equal(underscoreField.artifactBytes.length, 0);
  assert.equal(underscoreField.diagnostics.length, 1);
  assert.equal(underscoreField.diagnostics[0].message, "semantic error: unknown field '_' on type int");
  assert.equal(underscoreDigitField.artifactBytes.length, 0);
  assert.equal(underscoreDigitField.diagnostics.length, 1);
  assert.equal(underscoreDigitField.diagnostics[0].message, "semantic error: unknown field '__2' on type int");
  assert.equal(doubleDot.artifactBytes.length, 0);
  assert.equal(doubleDot.diagnostics.length, 1);
  assert.equal(doubleDot.diagnostics[0].message, "parser error: expected identifier or tuple index but found Dot");
  assert.equal(rightParen.artifactBytes.length, 0);
  assert.equal(rightParen.diagnostics.length, 1);
  assert.equal(rightParen.diagnostics[0].message, "parser error: expected identifier or tuple index but found RParen");
  assert.equal(oneLineMissingField.artifactBytes.length, 0);
  assert.equal(oneLineMissingField.diagnostics.length, 1);
  assert.equal(oneLineMissingField.diagnostics[0].message, "parser error: {error}: expected identifier or tuple index but found Semicolon");
});

test("Kotodama compiler SDK mirrors Rust expression-start parser diagnostics", () => {
  const compileStatement = (statement) => compileKotodamaProgram(`
seiyaku InvalidExpressionStart {
  kotoage fn run() permission(Admin) {
    ${statement}
  }
}
`);
  const semicolon = compileStatement('let value = ;');
  const lbrace = compileStatement('let value = {');
  const lbracket = compileStatement('let value = [;');
  const hash = compileStatement('let value = #;');
  const rbrace = compileStatement('let value = }');
  const oneLineSemicolon = compileKotodamaProgram("fn main() { let value = ; }");
  const oneLineLBracket = compileKotodamaProgram("fn main() { let value = [; }");
  const oneLineHash = compileKotodamaProgram("fn main() { let value = #; }");
  const oneLineBangEqual = compileKotodamaProgram("fn main() { let value = !=; }");
  const oneLineRBrace = compileKotodamaProgram("fn main() { let value = }");
  const oneLineEof = compileKotodamaProgram("fn main() { let value =");
  const oneLineBlockEof = compileKotodamaProgram("fn main() {");
  const multiLineBlockEof = compileKotodamaProgram("fn main() {\n  let value = 1;\n");

  assert.equal(semicolon.artifactBytes.length, 0);
  assert.equal(semicolon.diagnostics[0].message, "parser error: expected expression but found Semicolon");
  assert.equal(lbrace.artifactBytes.length, 0);
  assert.equal(lbrace.diagnostics[0].message, "parser error: {error}: expected expression but found LBrace");
  assert.equal(lbracket.artifactBytes.length, 0);
  assert.equal(lbracket.diagnostics[0].message, "parser error: expected expression but found LBracket");
  assert.equal(hash.artifactBytes.length, 0);
  assert.equal(hash.diagnostics[0].message, "parser error: expected expression but found Hash");
  assert.equal(rbrace.artifactBytes.length, 0);
  assert.equal(rbrace.diagnostics[0].message, "parser error: expected expression but found RBrace");
  assert.equal(oneLineSemicolon.artifactBytes.length, 0);
  assert.equal(oneLineSemicolon.diagnostics[0].message, "parser error: {error}: expected expression but found Semicolon");
  assert.equal(oneLineLBracket.artifactBytes.length, 0);
  assert.equal(oneLineLBracket.diagnostics[0].message, "parser error: {error}: expected expression but found LBracket");
  assert.equal(oneLineHash.artifactBytes.length, 0);
  assert.equal(oneLineHash.diagnostics[0].message, "parser error: {error}: expected expression but found Hash");
  assert.equal(oneLineBangEqual.artifactBytes.length, 0);
  assert.equal(oneLineBangEqual.diagnostics[0].message, "parser error: {error}: expected expression but found BangEqual");
  assert.equal(oneLineRBrace.artifactBytes.length, 0);
  assert.equal(oneLineRBrace.diagnostics[0].message, "parser error: {error}: expected expression but found RBrace");
  assert.equal(oneLineEof.artifactBytes.length, 0);
  assert.equal(oneLineEof.diagnostics[0].message, "parser error: {error}: expected expression but found EOF");
  assert.equal(oneLineBlockEof.artifactBytes.length, 0);
  assert.equal(oneLineBlockEof.diagnostics[0].message, "parser error: {error}: expected expression but found EOF");
  assert.equal(multiLineBlockEof.artifactBytes.length, 0);
  assert.equal(multiLineBlockEof.diagnostics[0].message, "parser error: expected expression but found EOF");
});

test("Kotodama compiler SDK mirrors Rust generic punctuation parser diagnostics", () => {
  const compileStatement = (statement) => compileKotodamaProgram(`
seiyaku InvalidPunctuation {
  kotoage fn run() permission(Admin) {
    ${statement}
  }
}
`);
  const missingEqual = compileStatement('let value 1;');
  const missingCallRParen = compileStatement('info(1;');
  const missingCallComma = compileStatement('min(1 2);');
  const bareIdentMissingSemicolon = compileStatement('value 1;');
  const indexedIdentMissingSemicolon = compileStatement('value[1] 2;');
  const pipeMissingSemicolon = compileStatement('let value = 1 | 2;');
  const ampersandMissingSemicolon = compileStatement('let value = 1 & 2;');
  const oneLineMissingEqual = compileKotodamaProgram("fn main() { let value 1; }");
  const oneLineMissingCallRParen = compileKotodamaProgram("fn main() { info(1; }");
  const oneLineMissingCallRParenAtEof = compileKotodamaProgram("fn main() { info(1");
  const oneLineMissingCallComma = compileKotodamaProgram("fn main() { min(1 2); }");
  const oneLineBareIdentMissingSemicolon = compileKotodamaProgram("fn main() { value 1; }");
  const oneLineIndexedIdentMissingSemicolon = compileKotodamaProgram("fn main() { value[1] 2; }");
  const oneLinePipeMissingSemicolon = compileKotodamaProgram("fn main() { let value = 1 | 2; }");
  const oneLineAmpersandMissingSemicolon = compileKotodamaProgram("fn main() { let value = 1 & 2; }");

  assert.equal(missingEqual.artifactBytes.length, 0);
  assert.equal(missingEqual.diagnostics[0].message, "parser error: expected Equal but found Number(1)");
  assert.equal(missingCallRParen.artifactBytes.length, 0);
  assert.equal(missingCallRParen.diagnostics[0].message, "parser error: expected RParen but found Semicolon");
  assert.equal(missingCallComma.artifactBytes.length, 0);
  assert.equal(missingCallComma.diagnostics[0].message, "parser error: expected RParen but found Number(2)");
  assert.equal(bareIdentMissingSemicolon.artifactBytes.length, 0);
  assert.equal(bareIdentMissingSemicolon.diagnostics[0].message, "parser error: expected Semicolon but found Number(1)");
  assert.equal(indexedIdentMissingSemicolon.artifactBytes.length, 0);
  assert.equal(indexedIdentMissingSemicolon.diagnostics[0].message, "parser error: expected Semicolon but found Number(2)");
  assert.equal(pipeMissingSemicolon.artifactBytes.length, 0);
  assert.equal(pipeMissingSemicolon.diagnostics[0].message, "parser error: expected Semicolon but found Pipe");
  assert.equal(ampersandMissingSemicolon.artifactBytes.length, 0);
  assert.equal(ampersandMissingSemicolon.diagnostics[0].message, "parser error: expected Semicolon but found Ampersand");
  assert.equal(oneLineMissingEqual.artifactBytes.length, 0);
  assert.equal(oneLineMissingEqual.diagnostics[0].message, "parser error: {error}: expected Equal but found Number(1)");
  assert.equal(oneLineMissingCallRParen.artifactBytes.length, 0);
  assert.equal(oneLineMissingCallRParen.diagnostics[0].message, "parser error: {error}: expected RParen but found Semicolon");
  assert.equal(oneLineMissingCallRParenAtEof.artifactBytes.length, 0);
  assert.equal(oneLineMissingCallRParenAtEof.diagnostics[0].message, "parser error: {error}: expected RParen but found EOF");
  assert.equal(oneLineMissingCallComma.artifactBytes.length, 0);
  assert.equal(oneLineMissingCallComma.diagnostics[0].message, "parser error: {error}: expected RParen but found Number(2)");
  assert.equal(oneLineBareIdentMissingSemicolon.artifactBytes.length, 0);
  assert.equal(oneLineBareIdentMissingSemicolon.diagnostics[0].message, "parser error: {error}: expected Semicolon but found Number(1)");
  assert.equal(oneLineIndexedIdentMissingSemicolon.artifactBytes.length, 0);
  assert.equal(oneLineIndexedIdentMissingSemicolon.diagnostics[0].message, "parser error: {error}: expected Semicolon but found Number(2)");
  assert.equal(oneLinePipeMissingSemicolon.artifactBytes.length, 0);
  assert.equal(
    oneLinePipeMissingSemicolon.diagnostics[0].message,
    "parser error: {error}: expected Semicolon but found Pipe",
  );
  assert.equal(oneLineAmpersandMissingSemicolon.artifactBytes.length, 0);
  assert.equal(
    oneLineAmpersandMissingSemicolon.diagnostics[0].message,
    "parser error: {error}: expected Semicolon but found Ampersand",
  );
  const operatorTailCases = [
    ['+=', 'PlusEqual'],
    ['-=', 'MinusEqual'],
    ['*=', 'StarEqual'],
    ['/=', 'SlashEqual'],
    ['%=', 'PercentEqual'],
    ['++', 'PlusPlus'],
  ];
  for (const [operator, tokenName] of operatorTailCases) {
    const compiled = compileKotodamaProgram(`fn main() { let value = 1 ${operator} 2; }`);
    assert.equal(compiled.artifactBytes.length, 0, operator);
    assert.equal(
      compiled.diagnostics[0].message,
      `parser error: {error}: expected Semicolon but found ${tokenName}`,
      operator,
    );
  }
});

test("Kotodama compiler SDK mirrors Rust declaration identifier parser diagnostics", () => {
  const functionName = compileKotodamaProgram(`
seiyaku InvalidFunctionName {
  kotoage fn 123() permission(Admin) {}
}
`);
  const seiyakuName = compileKotodamaProgram(`
seiyaku 123 {
  kotoage fn run() permission(Admin) {}
}
`);
  const structName = compileKotodamaProgram(`
seiyaku InvalidStructName {
  struct 123 { value: int; }
  kotoage fn run() permission(Admin) {}
}
`);
  const structField = compileKotodamaProgram(`
seiyaku InvalidStructField {
  struct User { 123: int; }
  kotoage fn run() permission(Admin) {}
}
`);
  const permissionName = compileKotodamaProgram(`
seiyaku InvalidPermissionName {
  kotoage fn run() permission("Admin") {}
}
`);
  const stateName = compileKotodamaProgram(`
seiyaku InvalidStateName {
  state 123: int;
  kotoage fn run() permission(Admin) {}
}
`);
  const topLevelMissingParam = compileKotodamaProgram("fn main(");
  const topLevelMissingParamAfterComma = compileKotodamaProgram("fn main(value: int,");
  const topLevelTypeThenNameEof = compileKotodamaProgram("fn main(int");
  const topLevelTypeThenComma = compileKotodamaProgram("fn main(int,");
  const contractMissingParam = compileKotodamaProgram("seiyaku Test { fn main(");
  const contractTypeThenNameEof = compileKotodamaProgram("seiyaku Test { fn main(int");
  const contractTypeThenComma = compileKotodamaProgram("seiyaku Test { fn main(int,");
  const bareDefaultParam = compileKotodamaProgram("fn main(value) -> int { return value; }");
  const stateParamMissingTypeName = compileKotodamaProgram("fn main(state value) {}");

  assert.equal(functionName.artifactBytes.length, 0);
  assert.equal(functionName.diagnostics[0].message, "parser error: {error}: expected identifier but found Number(123)");
  assert.equal(seiyakuName.artifactBytes.length, 0);
  assert.equal(seiyakuName.diagnostics[0].message, "parser error: {error}: expected identifier but found Number(123)");
  assert.equal(structName.artifactBytes.length, 0);
  assert.equal(structName.diagnostics[0].message, "parser error: {error}: expected identifier but found Number(123)");
  assert.equal(structField.artifactBytes.length, 0);
  assert.equal(structField.diagnostics[0].message, "parser error: {error}: expected identifier but found Number(123)");
  assert.equal(permissionName.artifactBytes.length, 0);
  assert.equal(permissionName.diagnostics[0].message, 'parser error: {error}: expected identifier but found String("Admin")');
  assert.equal(stateName.artifactBytes.length, 0);
  assert.equal(stateName.diagnostics[0].message, "parser error: expected identifier but found Number(123)");
  assert.equal(topLevelMissingParam.artifactBytes.length, 0);
  assert.equal(topLevelMissingParam.diagnostics[0].message, "parser error: expected identifier but found EOF");
  assert.equal(topLevelMissingParamAfterComma.artifactBytes.length, 0);
  assert.equal(topLevelMissingParamAfterComma.diagnostics[0].message, "parser error: expected identifier but found EOF");
  assert.equal(topLevelTypeThenNameEof.artifactBytes.length, 0);
  assert.equal(topLevelTypeThenNameEof.diagnostics[0].message, "parser error: expected RParen but found EOF");
  assert.equal(topLevelTypeThenComma.artifactBytes.length, 0);
  assert.equal(topLevelTypeThenComma.diagnostics[0].message, "parser error: expected identifier but found EOF");
  assert.equal(contractMissingParam.artifactBytes.length, 0);
  assert.equal(contractMissingParam.diagnostics[0].message, "parser error: {error}: expected identifier but found EOF");
  assert.equal(contractTypeThenNameEof.artifactBytes.length, 0);
  assert.equal(contractTypeThenNameEof.diagnostics[0].message, "parser error: {error}: expected RParen but found EOF");
  assert.equal(contractTypeThenComma.artifactBytes.length, 0);
  assert.equal(contractTypeThenComma.diagnostics[0].message, "parser error: {error}: expected identifier but found EOF");
  assert.ok(bareDefaultParam.artifactBytes.length > 0);
  assert.deepEqual(bareDefaultParam.diagnostics, []);
  assert.equal(stateParamMissingTypeName.artifactBytes.length, 0);
  assert.equal(stateParamMissingTypeName.diagnostics[0].message, "parser error: {error}: expected identifier but found State");
});

test("Kotodama compiler SDK mirrors Rust top-level item parser diagnostics", () => {
  const topIdentifier = compileKotodamaProgram("wat C {}\n");
  const topNumber = compileKotodamaProgram("123\n");
  const contractIdentifier = compileKotodamaProgram(`
seiyaku InvalidContractItem {
  wat C {}
}
`);
  const contractNumber = compileKotodamaProgram(`
seiyaku InvalidContractItem {
  123
}
`);
  const missingFn = compileKotodamaProgram(`
seiyaku InvalidContractItem {
  kotoage run() permission(Admin) {}
}
`);

  assert.equal(topIdentifier.artifactBytes.length, 0);
  assert.equal(topIdentifier.diagnostics[0].message, 'parser error: {error}: expected top-level item (fn, struct, state, seiyaku) but found Ident("wat")');
  assert.equal(topNumber.artifactBytes.length, 0);
  assert.equal(topNumber.diagnostics[0].message, "parser error: expected top-level item (fn, struct, state, seiyaku) but found Number(123)");
  assert.equal(contractIdentifier.artifactBytes.length, 0);
  assert.equal(contractIdentifier.diagnostics[0].message, 'parser error: {error}: expected contract item (fn, struct, const, state, meta) but found Ident("wat")');
  assert.equal(contractNumber.artifactBytes.length, 0);
  assert.equal(contractNumber.diagnostics[0].message, "parser error: expected contract item (fn, struct, const, state, meta) but found Number(123)");
  assert.equal(missingFn.artifactBytes.length, 0);
  assert.equal(missingFn.diagnostics[0].message, "parser error: {error}: expected contract item (fn, struct, const, state, meta) but found Kotoage");
});

test("Kotodama compiler SDK rejects invalid Iroha effect arguments semantically", () => {
  const invalidTransfer = compileKotodamaProgram(`
seiyaku InvalidTransfer {
  kotoage fn run() permission(Admin) {
    transfer_asset(authority(), authority(), name("rose"), 1);
  }
}
`);
  const invalidSetDetail = compileKotodamaProgram(`
seiyaku InvalidSetDetail {
  kotoage fn run() permission(Admin) {
    set_account_detail(authority(), 1, json!{ value: "ok" });
  }
}
`);
  const invalidMint = compileKotodamaProgram(`
seiyaku InvalidMint {
  kotoage fn run() permission(Admin) {
    mint_asset(authority(), name("rose"), 1);
  }
}
`);
const invalidTransferDomain = compileKotodamaProgram(`
seiyaku InvalidTransferDomain {
  kotoage fn run() permission(Admin) {
    transfer_domain(authority(), json_object(), authority());
  }
}
`);
  const invalidRegisterAccount = compileKotodamaProgram(`
seiyaku InvalidRegisterAccount {
  kotoage fn run() permission(Admin) {
    register_account(name("bad"));
  }
}
`);
  const invalidUnregisterAsset = compileKotodamaProgram(`
seiyaku InvalidUnregisterAsset {
  kotoage fn run() permission(Admin) {
    unregister_asset(name("rose"));
  }
}
`);
  const invalidRegisterAsset = compileKotodamaProgram(`
seiyaku InvalidRegisterAsset {
  kotoage fn run() permission(Admin) {
    register_asset(name("rose"), "ROSE", 1, 0);
  }
}
`);
  const invalidCreateNewAsset = compileKotodamaProgram(`
seiyaku InvalidCreateNewAsset {
  kotoage fn run() permission(Admin) {
    create_new_asset(asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), json!{ symbol: "ROSE" }, 1, authority(), 0);
  }
}
`);
  const invalidCreateRole = compileKotodamaProgram(`
seiyaku InvalidCreateRole {
  kotoage fn run() permission(Admin) {
    create_role(name("auditor"), name("read_blocks"));
  }
}
`);
  const invalidGrantPermission = compileKotodamaProgram(`
seiyaku InvalidGrantPermission {
  kotoage fn run() permission(Admin) {
    grant_permission(authority(), 1);
  }
}
`);
  const invalidRegisterPeer = compileKotodamaProgram(`
seiyaku InvalidRegisterPeer {
  kotoage fn run() permission(Admin) {
    register_peer(name("peer"));
  }
}
`);
  const invalidSetTriggerEnabled = compileKotodamaProgram(`
seiyaku InvalidSetTriggerEnabled {
  kotoage fn run() permission(Admin) {
    set_trigger_enabled(name("wake"), json!{ enabled: true });
  }
}
`);
  const invalidSubscription = compileKotodamaProgram(`
seiyaku InvalidSubscription {
  kotoage fn run() permission(Admin) {
    subscription_bill(1);
  }
}
`);
  const invalidTransferBatchBegin = compileKotodamaProgram(`
seiyaku InvalidTransferBatchBegin {
  kotoage fn run() permission(Admin) {
    transfer_v1_batch_begin(1);
  }
}
`);
  const invalidTransferBatchEmpty = compileKotodamaProgram(`
seiyaku InvalidTransferBatchEmpty {
  kotoage fn run() permission(Admin) {
    transfer_batch();
  }
}
`);
  const invalidTransferBatchEntry = compileKotodamaProgram(`
seiyaku InvalidTransferBatchEntry {
  kotoage fn run() permission(Admin) {
    transfer_batch(authority());
  }
}
`);
  const invalidCallContract = compileKotodamaProgram(`
seiyaku InvalidCallContract {
  kotoage fn run() permission(Admin) {
    let payload = json!{ amount: 1 };
    let response = call_contract(json_object(), "settle", payload);
  }
}
`);
  const invalidNftMint = compileKotodamaProgram(`
seiyaku InvalidNftMint {
  kotoage fn run() permission(Admin) {
    nft_mint_asset(name("bad"), authority());
  }
}
`);
  const invalidNftMetadata = compileKotodamaProgram(`
seiyaku InvalidNftMetadata {
  kotoage fn run() permission(Admin) {
    let nft = nft_id("n0$wonderland.universal");
    nft_set_metadata(nft, 1, json!{ "meta": 1 });
  }
}
`);
  const validEffects = compileKotodamaProgram(`
seiyaku ValidEffects {
  kotoage fn run() permission(Admin) {
    let asset = asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    let nft = nft_id("n0$wonderland.universal");
    transfer_asset(authority(), authority(), asset, 1);
    mint_asset(authority(), asset, 1);
    burn_asset(authority(), asset, 1);
    nft_mint_asset(nft, authority());
    nft_set_metadata(nft, name("dpn_metadata"), json!{ "meta": 1 });
    nft_transfer_asset(authority(), nft, authority());
    nft_burn_asset(nft);
    set_account_detail(authority(), name("status"), json!{ value: "ok" });
    transfer_domain(authority(), name("wonderland.universal"), authority());
    transfer_v1_batch_begin();
    transfer_v1_batch_end();
    transfer_batch((authority(), authority(), asset, 1));
  }
}
`);

  assert.equal(invalidTransfer.artifactBytes.length, 0);
  assert.equal(invalidTransfer.diagnostics.length, 1);
  assert.match(
    invalidTransfer.diagnostics[0].message,
    /transfer_asset expects \(AccountId, AccountId, AssetDefinitionId, numeric\)/,
  );
  assert.equal(invalidSetDetail.artifactBytes.length, 0);
  assert.equal(invalidSetDetail.diagnostics.length, 1);
  assert.match(invalidSetDetail.diagnostics[0].message, /set_account_detail expects \(AccountId, Name, Json\)/);
  assert.equal(invalidMint.artifactBytes.length, 0);
  assert.equal(invalidMint.diagnostics.length, 1);
  assert.match(invalidMint.diagnostics[0].message, /mint_asset expects \(AccountId, AssetDefinitionId, numeric\)/);
  assert.equal(invalidTransferDomain.artifactBytes.length, 0);
  assert.equal(invalidTransferDomain.diagnostics.length, 1);
  assert.match(
    invalidTransferDomain.diagnostics[0].message,
    /transfer_domain expects \(AccountId, DomainId, AccountId\)/,
  );
  assert.equal(invalidRegisterAccount.artifactBytes.length, 0);
  assert.equal(invalidRegisterAccount.diagnostics.length, 1);
  assert.match(invalidRegisterAccount.diagnostics[0].message, /register_account expects \(AccountId\)/);
  assert.equal(invalidUnregisterAsset.artifactBytes.length, 0);
  assert.equal(invalidUnregisterAsset.diagnostics.length, 1);
  assert.match(invalidUnregisterAsset.diagnostics[0].message, /unregister_asset expects \(AssetDefinitionId\)/);
  assert.equal(invalidRegisterAsset.artifactBytes.length, 0);
  assert.equal(invalidRegisterAsset.diagnostics.length, 1);
  assert.match(
    invalidRegisterAsset.diagnostics[0].message,
    /register_asset expects \(AssetDefinitionId, string, int, int\)/,
  );
  assert.equal(invalidCreateNewAsset.artifactBytes.length, 0);
  assert.equal(invalidCreateNewAsset.diagnostics.length, 1);
  assert.match(
    invalidCreateNewAsset.diagnostics[0].message,
    /create_new_asset expects \(AssetDefinitionId, string, int, AccountId, int\)/,
  );
  assert.equal(invalidCreateRole.artifactBytes.length, 0);
  assert.equal(invalidCreateRole.diagnostics.length, 1);
  assert.match(invalidCreateRole.diagnostics[0].message, /create_role expects \(Name, Json\)/);
  assert.equal(invalidGrantPermission.artifactBytes.length, 0);
  assert.equal(invalidGrantPermission.diagnostics.length, 1);
  assert.match(invalidGrantPermission.diagnostics[0].message, /grant\/revoke_permission expects \(AccountId, Name\|Json\)/);
  assert.equal(invalidRegisterPeer.artifactBytes.length, 0);
  assert.equal(invalidRegisterPeer.diagnostics.length, 1);
  assert.match(invalidRegisterPeer.diagnostics[0].message, /register_peer expects \(Json\)/);
  assert.equal(invalidSetTriggerEnabled.artifactBytes.length, 0);
  assert.equal(invalidSetTriggerEnabled.diagnostics.length, 1);
  assert.match(invalidSetTriggerEnabled.diagnostics[0].message, /set_trigger_enabled expects \(Name, int\)/);
  assert.equal(invalidSubscription.artifactBytes.length, 0);
  assert.equal(invalidSubscription.diagnostics.length, 1);
  assert.match(invalidSubscription.diagnostics[0].message, /subscription_bill expects no arguments/);
  assert.equal(invalidTransferBatchBegin.artifactBytes.length, 0);
  assert.equal(invalidTransferBatchBegin.diagnostics.length, 1);
  assert.match(invalidTransferBatchBegin.diagnostics[0].message, /transfer_v1_batch_begin expects no arguments/);
  assert.equal(invalidTransferBatchEmpty.artifactBytes.length, 0);
  assert.equal(invalidTransferBatchEmpty.diagnostics.length, 1);
  assert.match(invalidTransferBatchEmpty.diagnostics[0].message, /transfer_batch expects at least one entry/);
  assert.equal(invalidTransferBatchEntry.artifactBytes.length, 0);
  assert.equal(invalidTransferBatchEntry.diagnostics.length, 1);
  assert.match(
    invalidTransferBatchEntry.diagnostics[0].message,
    /transfer_batch expects \(AccountId, AccountId, AssetDefinitionId, numeric\) tuple entries/,
  );
  assert.equal(invalidCallContract.artifactBytes.length, 0);
  assert.equal(invalidCallContract.diagnostics.length, 1);
  assert.match(invalidCallContract.diagnostics[0].message, /call_contract expects \(String\|Blob, String\|Blob, Json\)/);
  assert.equal(invalidNftMint.artifactBytes.length, 0);
  assert.equal(invalidNftMint.diagnostics.length, 1);
  assert.match(invalidNftMint.diagnostics[0].message, /nft_mint_asset expects \(NftId, AccountId\)/);
  assert.equal(invalidNftMetadata.artifactBytes.length, 0);
  assert.equal(invalidNftMetadata.diagnostics.length, 1);
  assert.match(invalidNftMetadata.diagnostics[0].message, /nft_set_metadata expects \(NftId, Name, Json\)/);
  assert.deepEqual(validEffects.diagnostics, []);
  const validEffectsCode = readArtifactCode(validEffects.artifactBytes);
  assert.notEqual(validEffectsCode.indexOf(syscallNeedle(0x25)), -1);
  assert.notEqual(validEffectsCode.indexOf(syscallNeedle(0x26)), -1);
  assert.notEqual(validEffectsCode.indexOf(syscallNeedle(0x27)), -1);
  assert.notEqual(validEffectsCode.indexOf(syscallNeedle(0x28)), -1);
  assert.notEqual(validEffectsCode.indexOf(syscallNeedle(0x29)), -1);
  assert.notEqual(validEffectsCode.indexOf(syscallNeedle(0x24)), -1);
  assert.notEqual(validEffectsCode.indexOf(syscallNeedle(0x12)), -1);
  assert.notEqual(validEffectsCode.indexOf(syscallNeedle(0x2a)), -1);
});

test("Kotodama compiler SDK requires fully qualified static DomainId literals", () => {
  const invalidConstructor = compileKotodamaProgram(`
seiyaku InvalidDomainConstructor {
  kotoage fn run() permission(Admin) {
    register_domain(domain("wonderland"));
  }
}
`);
  const invalidTransferName = compileKotodamaProgram(`
seiyaku InvalidDomainTransferName {
  kotoage fn run() permission(Admin) {
    let domain_name = name("wonderland");
    transfer_domain(authority(), domain_name, authority());
  }
}
`);
  const invalidTriggerMatcher = compileKotodamaProgram(`
seiyaku InvalidDomainTriggerMatcher {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data domain created {
      domain "wonderland";
    }
  }
}
`);
  const validRuntimeName = compileKotodamaProgram(`
seiyaku ValidRuntimeDomainName {
  kotoage fn run(domain_name: Name) permission(Admin) {
    transfer_domain(authority(), domain_name, authority());
  }
}
`);

  assert.equal(invalidConstructor.artifactBytes.length, 0);
  assert.equal(invalidConstructor.diagnostics.length, 1);
  assert.match(invalidConstructor.diagnostics[0].message, /invalid DomainId literal `wonderland`/);
  assert.equal(invalidTransferName.artifactBytes.length, 0);
  assert.equal(invalidTransferName.diagnostics.length, 1);
  assert.match(invalidTransferName.diagnostics[0].message, /invalid DomainId literal `wonderland`/);
  assert.equal(invalidTriggerMatcher.artifactBytes.length, 0);
  assert.equal(invalidTriggerMatcher.diagnostics.length, 1);
  assert.match(
    invalidTriggerMatcher.diagnostics[0].message,
    /trigger `wake` has invalid `domain` matcher literal `wonderland` in `domain` data filter: domain id must use `domain\.dataspace` format/,
  );
  assert.equal(invalidTriggerMatcher.diagnostics[0].line, 8);
  assert.deepEqual(validRuntimeName.diagnostics, []);
  assert.notEqual(readArtifactCode(validRuntimeName.artifactBytes).indexOf(syscallNeedle(0x12)), -1);
});

test("Kotodama compiler SDK requires canonical AssetDefinitionId address literals", () => {
  const invalidHostCall = compileKotodamaProgram(`
seiyaku InvalidAssetDefinitionHostCall {
  kotoage fn run() permission(Admin) {
    mint_asset(authority(), asset_definition("rose#wonderland"), 1);
  }
}
`);
  const invalidUnusedLocal = compileKotodamaProgram(`
seiyaku InvalidAssetDefinitionLocal {
  kotoage fn run() permission(Admin) {
    let asset = asset_definition("rose#wonderland");
    info(1);
  }
}
`);
  const valid = compileKotodamaProgram(`
seiyaku ValidAssetDefinitionAddress {
  kotoage fn run() permission(Admin) {
    let asset = asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(authority(), asset, 1);
  }
}
`);

  assert.equal(invalidHostCall.artifactBytes.length, 0);
  assert.equal(invalidHostCall.diagnostics.length, 1);
  assert.match(invalidHostCall.diagnostics[0].message, /invalid AssetDefinitionId literal `rose#wonderland`/);
  assert.equal(invalidUnusedLocal.artifactBytes.length, 0);
  assert.equal(invalidUnusedLocal.diagnostics.length, 1);
  assert.match(invalidUnusedLocal.diagnostics[0].message, /invalid AssetDefinitionId literal `rose#wonderland`/);
  assert.deepEqual(valid.diagnostics, []);
  assert.notEqual(readArtifactCode(valid.artifactBytes).indexOf(syscallNeedle(0x22)), -1);
});

test("Kotodama compiler SDK mirrors Rust json macro key parsing", () => {
  const invalidReservedKey = compileKotodamaProgram(`
seiyaku InvalidJsonReservedKey {
  kotoage fn run() {
    let payload = json!{ meta: "ok" };
    info(1);
  }
}
`);
  const invalidThisKey = compileKotodamaProgram(`
seiyaku InvalidJsonThisKey {
  kotoage fn run() {
    let payload = json!{ this: 1 };
    info(1);
  }
}
`);
  const invalidFnKey = compileKotodamaProgram(`
seiyaku InvalidJsonFnKey {
  kotoage fn run() {
    let payload = json!{ fn: 1 };
    info(1);
  }
}
`);
  const invalidReturnKey = compileKotodamaProgram(`
seiyaku InvalidJsonReturnKey {
  kotoage fn run() {
    let payload = json!{ return: 1 };
    info(1);
  }
}
`);
  const invalidDuplicateKey = compileKotodamaProgram(`
seiyaku InvalidJsonDuplicateKey {
  kotoage fn run() {
    let payload = json!{ foo: 1, foo: 2 };
    info(1);
  }
}
`);
  const invalidTrailingObjectComma = compileKotodamaProgram(`
seiyaku InvalidJsonTrailingObjectComma {
  kotoage fn run() {
    let payload = json!{ ok: true, };
    info(1);
  }
}
`);
  const invalidTrailingArrayComma = compileKotodamaProgram(`
seiyaku InvalidJsonTrailingArrayComma {
  kotoage fn run() {
    let payload = json![1, 2, ];
    info(1);
  }
}
`);
  const invalidNegativeBoolean = compileKotodamaProgram(`
seiyaku InvalidJsonNegativeBoolean {
  kotoage fn run() {
    let payload = json!{ ok: -true };
    info(1);
  }
}
`);
  const invalidBangWithoutParen = compileKotodamaProgram(`
seiyaku InvalidJsonBangWithoutParen {
  kotoage fn run() {
    let payload = json!42;
    info(1);
  }
}
`);
  const invalidBangStringWithoutParen = compileKotodamaProgram(`
seiyaku InvalidJsonBangStringWithoutParen {
  kotoage fn run() {
    let payload = json!"x";
    info(1);
  }
}
`);
  const invalidPositiveOverflow = compileKotodamaProgram(`
seiyaku InvalidJsonPositiveOverflow {
  kotoage fn run() {
    let payload = json!{ value: 9223372036854775808 };
    info(1);
  }
}
`);
  const invalidNegativeOverflow = compileKotodamaProgram(`
seiyaku InvalidJsonNegativeOverflow {
  kotoage fn run() {
    let payload = json!{ value: -9223372036854775809 };
    info(1);
  }
}
`);
  const invalidDecimal = compileKotodamaProgram(`
seiyaku InvalidJsonDecimal {
  kotoage fn run() {
    let payload = json!{ value: 1.5 };
    info(1);
  }
}
`);
  const invalidNegativeDecimal = compileKotodamaProgram(`
seiyaku InvalidJsonNegativeDecimal {
  kotoage fn run() {
    let payload = json!{ value: -1.5 };
    info(1);
  }
}
`);
  const validI64Bounds = compileKotodamaProgram(`
seiyaku ValidJsonI64Bounds {
  kotoage fn run() {
    let payload = json!{ min: -9223372036854775808, max: 9223372036854775807 };
    info(1);
  }
}
`);
  const validQuotedReservedKey = compileKotodamaProgram(`
seiyaku ValidJsonQuotedReservedKey {
  kotoage fn run() {
    let payload = json!{ "meta": "ok", null: 1 };
    info(1);
  }
}
`);

  assert.equal(invalidReservedKey.artifactBytes.length, 0);
  assert.equal(invalidReservedKey.diagnostics.length, 1);
  assert.equal(invalidReservedKey.diagnostics[0].message, 'parser error: {error}: expected json! object keys must be identifiers or string literals but found Meta');
  assert.equal(invalidThisKey.diagnostics[0].message, 'parser error: {error}: expected json! object keys must be identifiers or string literals but found This');
  assert.equal(invalidFnKey.diagnostics[0].message, 'parser error: {error}: expected json! object keys must be identifiers or string literals but found Fn');
  assert.equal(invalidReturnKey.diagnostics[0].message, 'parser error: {error}: expected json! object keys must be identifiers or string literals but found Return');
  assert.equal(invalidDuplicateKey.artifactBytes.length, 0);
  assert.equal(invalidDuplicateKey.diagnostics.length, 1);
  assert.match(invalidDuplicateKey.diagnostics[0].message, /duplicate key in `json!\{\}` object literal/);
  assert.equal(invalidTrailingObjectComma.diagnostics[0].message, "parser error: {error}: expected json! object keys must be identifiers or string literals but found RBrace");
  assert.equal(invalidTrailingArrayComma.diagnostics[0].message, "parser error: {error}: expected unsupported value in `json!{}` macro but found RBracket");
  assert.equal(invalidNegativeBoolean.diagnostics[0].message, "parser error: {error}: expected expected number after '-' in json! literal but found True");
  assert.equal(invalidBangWithoutParen.diagnostics[0].message, "parser error: expected LParen but found Number(42)");
  assert.equal(invalidBangStringWithoutParen.diagnostics[0].message, 'parser error: expected LParen but found String("x")');
  assert.equal(invalidPositiveOverflow.diagnostics[0].message, "parser error: {error}: integer literal out of range (max 9223372036854775807)");
  assert.equal(invalidNegativeOverflow.diagnostics[0].message, "parser error: {error}: integer literal out of range (min -9223372036854775808)");
  assert.equal(invalidDecimal.diagnostics[0].message, 'parser error: {error}: expected unsupported value in `json!{}` macro but found Decimal("1.5")');
  assert.equal(invalidNegativeDecimal.diagnostics[0].message, 'parser error: {error}: expected expected number after \'-\' in json! literal but found Decimal("1.5")');
  assert.deepEqual(validQuotedReservedKey.diagnostics, []);
  assert.deepEqual(validI64Bounds.diagnostics, []);
});

test("Kotodama compiler SDK mirrors Rust prelude macro parser diagnostics", () => {
  const unknownMacro = compileKotodamaProgram(`
seiyaku UnknownPreludeMacro {
  kotoage fn run() {
    let value = foo!("x");
    info(1);
  }
}
`);
  const invalidArgCount = compileKotodamaProgram(`
seiyaku InvalidPreludeMacroArgCount {
  kotoage fn run() {
    let value = name!("a", "b");
    info(1);
  }
}
`);
  const invalidArgType = compileKotodamaProgram(`
seiyaku InvalidPreludeMacroArgType {
  kotoage fn run() {
    let value = name!(1);
    info(1);
  }
}
`);
  const invalidNoParen = compileKotodamaProgram(`
seiyaku InvalidPreludeMacroNoParen {
  kotoage fn run() {
    let value = name!"x";
    info(1);
  }
}
`);
  const valid = compileKotodamaProgram(`
seiyaku ValidPreludeMacro {
  kotoage fn run() {
    let value = name!("x");
    info(1);
  }
}
`);

  assert.equal(unknownMacro.diagnostics[0].message, 'parser error: {error}: expected unknown macro; supported prelude macros: account!, account_id!, asset_definition!, asset_id!, domain!, domain_id!, name!, json!, nft_id!, blob!, norito_bytes! but found Ident("foo")');
  assert.equal(invalidArgCount.diagnostics[0].message, 'parser error: {error}: expected prelude macro expects a single string literal argument but found Ident("name")');
  assert.equal(invalidArgType.diagnostics[0].message, 'parser error: {error}: expected prelude macro expects a string literal argument but found Ident("name")');
  assert.equal(invalidNoParen.diagnostics[0].message, 'parser error: {error}: expected LParen but found String("x")');
  assert.deepEqual(valid.diagnostics, []);
});

test("Kotodama compiler SDK treats json constructor strings as raw JSON like Rust", () => {
  const valid = compileKotodamaProgram(`
fn main() {
  let raw_object: String = "{\\"ok\\":true}";
  let raw_blob = blob("0x7b7d");
  let raw_norito = norito_bytes("0x7b7d");
  set_account_detail(authority(), name("object"), json(raw_object));
  set_account_detail(authority(), name("string"), json("\\"ok\\""));
  set_account_detail(authority(), name("blob"), json(raw_blob));
  set_account_detail(authority(), name("norito"), json(raw_norito));
}
`);
  const invalidDirect = compileKotodamaProgram(`
fn main() {
  let payload = json("hello");
}
`);
  const invalidLocal = compileKotodamaProgram(`
fn main() {
  let raw: String = "hello";
  set_account_detail(authority(), name("raw"), json(raw));
}
`);
  const invalidConst = compileKotodamaProgram(`
const RAW: String = "hello";
fn main() {
  set_account_detail(authority(), name("raw"), json(RAW));
}
`);
  const invalidMacro = compileKotodamaProgram(`
fn main() {
  set_account_detail(authority(), name("raw"), json!("hello"));
}
`);

  assert.equal(valid.diagnostics.length, 0);
  assert.notEqual(readArtifactCode(valid.artifactBytes).indexOf(syscallNeedle(0x58)), -1);
  assert.match(invalidDirect.diagnostics[0].message, /invalid JSON literal `hello`/);
  assert.match(invalidLocal.diagnostics[0].message, /invalid JSON literal `hello`/);
  assert.match(invalidConst.diagnostics[0].message, /invalid JSON literal `hello`/);
  assert.match(invalidMacro.diagnostics[0].message, /invalid JSON literal `hello`/);
});

test("Kotodama compiler SDK accepts NoritoBytes in blob constructors like Rust", () => {
  const compiled = compileKotodamaProgram(`
seiyaku BlobFromNoritoBytes {
  kotoage fn run() permission(Admin) {
    let raw = norito_bytes("0x0102");
    let wrapped = blob(raw);
    info(tlv_len(wrapped));
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
});

test("Kotodama compiler SDK accepts inline NoritoBytes in DomainId and NftId constructors like Rust", () => {
  const compiled = compileKotodamaProgram(`
seiyaku PointerConstructorsFromInlineNorito {
  state DomainId LastDomain;
  state NftId LastNft;

  kotoage fn run() permission(Admin) {
    let domain_from_inline: DomainId = domain_id(norito_bytes("0x0102"));
    let nft_from_inline: NftId = nft_id(norito_bytes("0x0304"));
    let raw = norito_bytes("0x0506");
    let domain_from_local: DomainId = domain_id(raw);
    LastDomain = domain_from_inline;
    LastDomain = domain_from_local;
    LastNft = nft_from_inline;
    info(tlv_len(pointer_to_norito(domain_from_inline)) + tlv_len(pointer_to_norito(nft_from_inline)));
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(syscallNeedle(0x5e)), -1, "missing POINTER_FROM_NORITO syscall");
  assert.deepEqual(compiled.manifest?.states, [
    { name: "LastDomain", type_name: "DomainId" },
    { name: "LastNft", type_name: "NftId" },
  ]);
});

test("Kotodama compiler SDK rejects invalid String ABI coercions", () => {
  const invalidJsonAssignment = compileKotodamaProgram(`
seiyaku InvalidStringJson {
  fn helper() {
    let payload: Json = "plain";
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidDynamicName = compileKotodamaProgram(`
seiyaku InvalidDynamicStringConstructor {
  fn label(raw: String) -> String {
    return raw;
  }

  fn helper(raw: String) {
    let key = name(label(raw));
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(invalidJsonAssignment.artifactBytes.length, 0);
  assert.equal(invalidJsonAssignment.diagnostics.length, 1);
  assert.match(
    invalidJsonAssignment.diagnostics[0].message,
    /type annotation mismatch: expected Json, got string/,
  );
  assert.equal(invalidDynamicName.artifactBytes.length, 0);
  assert.equal(invalidDynamicName.diagnostics.length, 1);
  assert.match(
    invalidDynamicName.diagnostics[0].message,
    /name expects a string literal; pass a literal or Blob\|bytes payload/,
  );
});

test("Kotodama compiler SDK accepts literal String helper returns in pointer constructors", () => {
  const compiled = compileKotodamaProgram(`
seiyaku LiteralStringPointerConstructors {
  fn asset_literal() -> String {
    return "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
  }

  fn domain_literal() -> String {
    let domain_name = "wonderland.universal";
    return domain_name;
  }

  fn pass(raw: String) -> String {
    return raw;
  }

  fn asset_from_param(raw: String) -> AssetDefinitionId {
    return asset_definition(raw);
  }

  kotoage fn run() permission(Admin) {
    let owner = account_id(pass("merchant@paynet"));
    let asset = asset_definition(asset_literal());
    let second_asset = asset_from_param("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    let domain_id = domain(domain_literal());
    let key = name(pass("memo"));
    let payload = json(pass("{\\"ok\\":true}"));
    mint_asset(owner, asset, 1);
    burn_asset(owner, second_asset, 1);
    register_domain(domain_id);
    set_account_detail(owner, key, payload);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  const code = readArtifactCode(compiled.artifactBytes);
  assert.notEqual(code.indexOf(syscallNeedle(0x22)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x23)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x10)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x1a)), -1);
});

test("Kotodama compiler SDK mirrors Rust helper-derived String constructor validation edge cases", () => {
  const helperDerivedInvalid = compileKotodamaProgram(`
seiyaku HelperDerivedInvalidStrings {
  fn bad_asset() -> String {
    return "rose#wonderland";
  }

  fn bad_domain() -> String {
    let raw = "wonderland";
    return raw;
  }

  fn bad_account() -> String {
    return "merchant";
  }

  fn bad_name() -> String {
    return "bad name";
  }

  fn pass(raw: String) -> String {
    return raw;
  }

  fn asset_from_param(raw: String) -> AssetDefinitionId {
    return asset_definition(raw);
  }

  kotoage fn run() permission(Admin) {
    let raw_asset = bad_asset();
    let raw_domain = bad_domain();
    let raw_json = pass("hello");
    let second_asset = asset_from_param("rose#wonderland");
    mint_asset(account_id(bad_account()), asset_definition(raw_asset), 1);
    burn_asset(authority(), second_asset, 1);
    register_domain(domain(raw_domain));
    set_account_detail(account_id(bad_account()), name(bad_name()), json(raw_json));
  }
}
`);
  const invalidNameLocal = compileKotodamaProgram(`
fn main() {
  let raw: String = "bad name";
  set_account_detail(authority(), name(raw), json("{}"));
}
`);

  assert.deepEqual(helperDerivedInvalid.diagnostics, []);
  const code = readArtifactCode(helperDerivedInvalid.artifactBytes);
  assert.notEqual(code.indexOf(syscallNeedle(0x23)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x10)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x1a)), -1);
  assert.equal(invalidNameLocal.artifactBytes.length, 0);
  assert.equal(invalidNameLocal.diagnostics.length, 1);
  assert.match(invalidNameLocal.diagnostics[0].message, /invalid Name literal `bad name`/);
});

test("Kotodama compiler SDK lowers transfer_batch entries between V1 batch boundaries", () => {
  const compiled = compileKotodamaProgram(`
seiyaku TransferBatch {
  kotoage fn run() permission(Admin) {
    let asset = asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    transfer_batch(
      (authority(), authority(), asset, 7),
      (authority(), authority(), asset, 3)
    );
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  const code = readArtifactCode(compiled.artifactBytes);
  const batchBegin = code.indexOf(syscallNeedle(0x29));
  const firstTransfer = code.indexOf(syscallNeedle(0x24));
  const secondTransfer = code.indexOf(syscallNeedle(0x24), firstTransfer + 1);
  const batchEnd = code.indexOf(syscallNeedle(0x2a));
  assert.notEqual(batchBegin, -1);
  assert.notEqual(firstTransfer, -1);
  assert.notEqual(secondTransfer, -1);
  assert.notEqual(batchEnd, -1);
  assert.equal(code.indexOf(syscallNeedle(0x24), secondTransfer + 1), -1);
  assert.ok(batchBegin < firstTransfer);
  assert.ok(firstTransfer < secondTransfer);
  assert.ok(secondTransfer < batchEnd);
});

test("Kotodama compiler SDK emits AXT pointer constructors and syscalls", () => {
  const compiled = compileKotodamaProgram(`
seiyaku AxtIntrinsics {
  kotoage fn run() permission(Admin) {
    let ds = dataspace_id("7");
    let desc = axt_descriptor(norito_bytes("0x00"));
    let handle = asset_handle(norito_bytes("0x00"));
    let proof = proof_blob(norito_bytes("0x00"));
    axt_begin(desc);
    axt_touch(ds, norito_bytes("manifest"));
    axt_touch(ds);
    verify_ds_proof(ds, proof);
    verify_ds_proof(ds);
    use_asset_handle(handle, norito_bytes("intent"), proof);
    use_asset_handle(handle, norito_bytes("intent"));
    axt_commit();
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  const code = readArtifactCode(compiled.artifactBytes);
  assert.notEqual(code.indexOf(syscallNeedle(0xb0)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0xb1)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0xb2)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0xb3)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0xb4)), -1);
  const runBudget = compiled.budgetReport.find((entry) => entry.function_name === "run");
  assert.deepEqual(
    {
      bytecode_bytes: runBudget?.bytecode_bytes,
      frame_bytes: runBudget?.frame_bytes,
    },
    {
      bytecode_bytes: 1172,
      frame_bytes: 48,
    },
  );
  assert.equal(compiled.manifest.entrypoints[0].access_hints_complete, false);
  assert.deepEqual(compiled.manifest.entrypoints[0].access_hints_skipped, [
    "opaque ISI access is not compiler-resolved",
  ]);
  assert.equal(readArtifactSection(compiled.artifactBytes, 17).length, 231);
  assert.equal(compiled.artifactBytes.length, 1697);
  assert.equal(code.indexOf(ivmWordNeedle(ivmWord(0x20, 12, 10, 0))), -1);
  assert.equal(code.indexOf(ivmWordNeedle(ivmWord(0x20, 13, 10, 0))), -1);
  assert.equal(code.indexOf(ivmWordNeedle(ivmWord(0x20, 10, 12, 0))), -1);
});

test("Kotodama compiler SDK supports AXT pointer durable state keys and values", () => {
  const compiled = compileKotodamaProgram(`
seiyaku AxtState {
  state HandlesByDescriptor: Map<AxtDescriptor, AssetHandle>;
  state ScoresByProof: Map<ProofBlob, int>;
  state LastDescriptor: AxtDescriptor;
  state LastHandle: AssetHandle;
  state LastProof: ProofBlob;

  kotoage fn run() permission(Admin) {
    let desc = axt_descriptor(norito_bytes("0x00"));
    let handle = asset_handle(norito_bytes("0x00"));
    let proof = proof_blob(norito_bytes("0x00"));
    HandlesByDescriptor[desc] = handle;
    ScoresByProof[proof] = 7;
    LastDescriptor = desc;
    LastHandle = HandlesByDescriptor[desc];
    LastProof = proof;
    let _score = ScoresByProof.get_or(proof, 0);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  const code = readArtifactCode(compiled.artifactBytes);
  assert.notEqual(code.indexOf(syscallNeedle(0x50)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x51)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x56)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x5d)), -1);
  assert.notEqual(code.indexOf(syscallNeedle(0x5e)), -1);
});

test("Kotodama compiler SDK rejects invalid static DataSpaceId and AXT pointer literal strings like Rust", () => {
  const encodedDataspace7 =
    "0x4e52543000003d41714fe9a3947921daca6132402f7a000900000000000000c5b0dbe4c15a440602080700000000000000";
  const validDataspace = compileKotodamaProgram(`
seiyaku ValidDataSpaceIdPointer {
  kotoage fn run() permission(Admin) {
    let ds = dataspace_id("7");
    axt_touch(ds);
  }
}
`);
  const validEncodedDataspace = compileKotodamaProgram(`
seiyaku ValidEncodedDataSpaceIdPointer {
  kotoage fn run() permission(Admin) {
    axt_touch(dataspace_id("${encodedDataspace7}"));
  }
}
`);
  const validBlobLikeAxt = compileKotodamaProgram(`
seiyaku ValidAxtBlobLikePointers {
  kotoage fn run() permission(Admin) {
    let desc = axt_descriptor(norito_bytes("0x00"));
    let handle = asset_handle(norito_bytes("0x00"));
    let proof = proof_blob(norito_bytes("0x00"));
    axt_begin(desc);
    verify_ds_proof(dataspace_id("7"), proof);
    use_asset_handle(handle, norito_bytes("intent"));
  }
}
`);
  const invalidDataspace = compileKotodamaProgram(`
seiyaku InvalidDataSpaceIdPointer {
  kotoage fn run() permission(Admin) {
    let ds = dataspace_id("x");
    axt_touch(ds);
  }
}
`);
  const invalidDescriptor = compileKotodamaProgram(`
seiyaku InvalidAxtDescriptorPointer {
  kotoage fn run() permission(Admin) {
    let desc = axt_descriptor("0x00");
    axt_begin(desc);
  }
}
`);
  const invalidHandle = compileKotodamaProgram(`
seiyaku InvalidAssetHandlePointer {
  kotoage fn run() permission(Admin) {
    let handle = asset_handle("0x00");
    use_asset_handle(handle, norito_bytes("intent"));
  }
}
`);
  const invalidProof = compileKotodamaProgram(`
seiyaku InvalidProofBlobPointer {
  kotoage fn run() permission(Admin) {
    let proof = proof_blob("0x00");
    verify_ds_proof(dataspace_id("7"), proof);
  }
}
`);

  assert.deepEqual(validDataspace.diagnostics, []);
  const dataspaceEntry = readLiteralTableEntries(validDataspace.artifactBytes).find((entry) => entry.type === 0x000a);
  assert.equal(dataspaceEntry?.length, 49);
  assert.equal(dataspaceEntry?.payload.subarray(0, 4).toString("ascii"), "NRT0");
  assert.equal(dataspaceEntry?.payload.toString("hex"), encodedDataspace7.slice(2));
  assert.deepEqual(validEncodedDataspace.diagnostics, []);
  const encodedDataspaceEntry = readLiteralTableEntries(validEncodedDataspace.artifactBytes).find((entry) => entry.type === 0x000a);
  assert.equal(encodedDataspaceEntry?.payload.toString("hex"), encodedDataspace7.slice(2));
  assert.deepEqual(validBlobLikeAxt.diagnostics, []);
  assert.equal(invalidDataspace.artifactBytes.length, 0);
  assert.match(invalidDataspace.diagnostics[0].message, /invalid DataSpaceId literal `x`: cannot decode/);
  assert.equal(invalidDescriptor.artifactBytes.length, 0);
  assert.match(invalidDescriptor.diagnostics[0].message, /invalid AxtDescriptor literal `0x00`: cannot decode/);
  assert.equal(invalidHandle.artifactBytes.length, 0);
  assert.match(invalidHandle.diagnostics[0].message, /invalid AssetHandle literal `0x00`: cannot decode/);
  assert.equal(invalidProof.artifactBytes.length, 0);
  assert.match(invalidProof.diagnostics[0].message, /invalid ProofBlob literal `0x00`: cannot decode/);
});

test("Kotodama compiler SDK rejects invalid AXT arguments semantically", () => {
  const invalidBegin = compileKotodamaProgram(`
seiyaku InvalidAxtBegin {
  kotoage fn run() permission(Admin) {
    axt_begin(norito_bytes("0x00"));
  }
}
`);
  const invalidTouch = compileKotodamaProgram(`
seiyaku InvalidAxtTouch {
  kotoage fn run() permission(Admin) {
    axt_touch(dataspace_id("7"), 1);
  }
}
`);
  const invalidProof = compileKotodamaProgram(`
seiyaku InvalidVerifyDsProof {
  kotoage fn run() permission(Admin) {
    verify_ds_proof(dataspace_id("7"), norito_bytes("0x00"));
  }
}
`);
  const invalidHandle = compileKotodamaProgram(`
seiyaku InvalidUseAssetHandle {
  kotoage fn run() permission(Admin) {
    use_asset_handle(asset_handle(norito_bytes("0x00")), 1);
  }
}
`);
  const invalidCommit = compileKotodamaProgram(`
seiyaku InvalidAxtCommit {
  kotoage fn run() permission(Admin) {
    axt_commit(1);
  }
}
`);

  assert.equal(invalidBegin.artifactBytes.length, 0);
  assert.match(invalidBegin.diagnostics[0].message, /axt_begin expects \(AxtDescriptor\)/);
  assert.equal(invalidTouch.artifactBytes.length, 0);
  assert.match(invalidTouch.diagnostics[0].message, /axt_touch expects \(DataSpaceId\[, Blob\|bytes manifest\]\)/);
  assert.equal(invalidProof.artifactBytes.length, 0);
  assert.match(invalidProof.diagnostics[0].message, /verify_ds_proof expects \(DataSpaceId\[, ProofBlob\]\)/);
  assert.equal(invalidHandle.artifactBytes.length, 0);
  assert.match(invalidHandle.diagnostics[0].message, /use_asset_handle expects \(AssetHandle, Blob\|bytes intent\[, ProofBlob\]\)/);
  assert.equal(invalidCommit.artifactBytes.length, 0);
  assert.match(invalidCommit.diagnostics[0].message, /axt_commit expects no arguments/);
});

test("Kotodama compiler SDK rejects invalid native escrow arguments semantically", () => {
  const invalidOpenOfferAsset = compileKotodamaProgram(`
seiyaku InvalidEscrowOpenOfferAsset {
  kotoage fn run() permission(Admin) {
    escrow_open_offer(name("deal"), name("rose"), 10);
  }
}
`);
  const invalidAcceptId = compileKotodamaProgram(`
seiyaku InvalidEscrowAcceptId {
  kotoage fn run() permission(Admin) {
    escrow_accept(1);
  }
}
`);
  const invalidDisputeEvidence = compileKotodamaProgram(`
seiyaku InvalidEscrowDisputeEvidence {
  kotoage fn run() permission(Admin) {
    escrow_open_dispute(name("deal"), 1);
  }
}
`);
  const invalidResolveAmount = compileKotodamaProgram(`
seiyaku InvalidEscrowResolveAmount {
  kotoage fn run() permission(Admin) {
    escrow_resolve_dispute(name("deal"), name("buyer"), 4);
  }
}
`);
  const validEscrow = compileKotodamaProgram(`
seiyaku ValidEscrowSemantic {
  kotoage fn run() permission(Admin) {
    let evidence = norito_bytes("00");
    escrow_open_offer(name("aitai_offer"), asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), 10, evidence);
    escrow_accept(name("aitai_offer"));
    escrow_mark_payment_sent(name("aitai_offer"));
    escrow_release(name("aitai_offer"));
    escrow_cancel(name("aitai_offer"));
    escrow_open_dispute(name("aitai_offer"), evidence);
    escrow_resolve_dispute(name("aitai_offer"), 6, 4, evidence);
  }
}
`);

  assert.equal(invalidOpenOfferAsset.artifactBytes.length, 0);
  assert.equal(invalidOpenOfferAsset.diagnostics.length, 1);
  assert.match(
    invalidOpenOfferAsset.diagnostics[0].message,
    /escrow_open_offer expects \(Name, AssetDefinitionId, numeric\[, Blob\|bytes evidence_hashes\]\)/,
  );
  assert.equal(invalidAcceptId.artifactBytes.length, 0);
  assert.equal(invalidAcceptId.diagnostics.length, 1);
  assert.match(invalidAcceptId.diagnostics[0].message, /escrow_accept expects \(Name\)/);
  assert.equal(invalidDisputeEvidence.artifactBytes.length, 0);
  assert.equal(invalidDisputeEvidence.diagnostics.length, 1);
  assert.match(
    invalidDisputeEvidence.diagnostics[0].message,
    /escrow_open_dispute expects \(Name\[, Blob\|bytes evidence_hashes\]\)/,
  );
  assert.equal(invalidResolveAmount.artifactBytes.length, 0);
  assert.equal(invalidResolveAmount.diagnostics.length, 1);
  assert.match(
    invalidResolveAmount.diagnostics[0].message,
    /escrow_resolve_dispute expects \(Name, numeric, numeric\[, Blob\|bytes evidence_hashes\]\)/,
  );
  assert.deepEqual(validEscrow.diagnostics, []);
});

test("Kotodama compiler SDK rejects invalid inline ZK builder arguments semantically", () => {
  const invalidSubmit = compileKotodamaProgram(`
seiyaku InvalidSubmitBuilder {
  kotoage fn run() permission(Admin) {
    let bytes = build_submit_ballot_inline("election", 1, blob("0123456789abcdef0123456789abcdef"), "halo2", blob("proof"), blob("vk"));
    execute_instruction(bytes);
  }
}
`);
  const invalidUnshield = compileKotodamaProgram(`
seiyaku InvalidUnshieldBuilder {
  kotoage fn run() permission(Admin) {
    let bytes = build_unshield_inline(name("asset"), authority(), 1, blob("0123456789abcdef0123456789abcdef"), "halo2", blob("proof"), blob("vk"));
    execute_instruction(bytes);
  }
}
`);
  const invalidUnshieldProof = compileKotodamaProgram(`
seiyaku InvalidUnshieldProof {
  kotoage fn run() permission(Admin) {
    let asset = asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    let bytes = build_unshield_inline(asset, authority(), 1, blob("0123456789abcdef0123456789abcdef"), "halo2", 1, blob("vk"));
    execute_instruction(bytes);
  }
}
`);
  const dynamicUnshieldAmount = compileKotodamaProgram(`
seiyaku DynamicUnshieldAmount {
  kotoage fn run(amount: Amount) permission(Admin) {
    let asset = asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    let bytes = build_unshield_inline(asset, authority(), amount, blob("0123456789abcdef0123456789abcdef"), "halo2", blob("proof"), blob("vk"));
    execute_instruction(bytes);
  }
}
`);

  assert.equal(invalidSubmit.artifactBytes.length, 0);
  assert.equal(invalidSubmit.diagnostics.length, 1);
  assert.match(
    invalidSubmit.diagnostics[0].message,
    /build_submit_ballot_inline expects \(string election_id, Blob\|bytes ciphertext, Blob\|bytes nullifier32, string backend, Blob\|bytes proof, Blob\|bytes vk\)/,
  );
  assert.equal(invalidUnshield.artifactBytes.length, 0);
  assert.equal(invalidUnshield.diagnostics.length, 1);
  assert.match(
    invalidUnshield.diagnostics[0].message,
    /build_unshield_inline expects \(AssetDefinitionId, AccountId, int amount, Blob\|bytes inputs32, string backend, Blob\|bytes proof, Blob\|bytes vk\)/,
  );
  assert.equal(invalidUnshieldProof.artifactBytes.length, 0);
  assert.equal(invalidUnshieldProof.diagnostics.length, 1);
  assert.match(
    invalidUnshieldProof.diagnostics[0].message,
    /build_unshield_inline expects \(AssetDefinitionId, AccountId, int amount, Blob\|bytes inputs32, string backend, Blob\|bytes proof, Blob\|bytes vk\)/,
  );
  assert.equal(dynamicUnshieldAmount.artifactBytes.length, 0);
  assert.equal(dynamicUnshieldAmount.diagnostics.length, 1);
  assert.match(
    dynamicUnshieldAmount.diagnostics[0].message,
    /build_unshield_inline amount requires a compile-time integer literal/,
  );
});

test("Kotodama compiler SDK rejects non-unit functions without all-path returns", () => {
  const compiled = compileKotodamaProgram(`
seiyaku MissingReturn {
  view fn run(flag: bool) -> int {
    if flag {
      return 1;
    }
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(compiled.diagnostics[0].message, /not all paths return a value/);
});

test("Kotodama compiler SDK rejects return values without declared return type", () => {
  const direct = compileKotodamaProgram(`
seiyaku UnexpectedDirectReturnValue {
  kotoage fn run() {
    return 1;
  }
}
`);
  const nested = compileKotodamaProgram(`
seiyaku UnexpectedNestedReturnValue {
  kotoage fn run() {
    if true {
      return 1;
    }
  }
}
`);

  assert.equal(direct.artifactBytes.length, 0);
  assert.equal(direct.diagnostics.length, 1);
  assert.match(direct.diagnostics[0].message, /returning a value requires a declared return type/);
  assert.equal(nested.artifactBytes.length, 0);
  assert.equal(nested.diagnostics.length, 1);
  assert.match(nested.diagnostics[0].message, /returning a value requires a declared return type/);
});

test("Kotodama compiler SDK preserves explicit unit return semantics", () => {
  const namedUnit = compileKotodamaProgram(`
seiyaku NamedUnitReturn {
  fn helper() -> unit {
    return;
  }

  kotoage fn run() permission(Admin) {
    helper();
  }
}
`);
  const tupleUnit = compileKotodamaProgram(`
seiyaku TupleUnitReturn {
  fn helper() -> () {
    return;
  }

  kotoage fn run() permission(Admin) {
    helper();
  }
}
`);
  const invalidNamedValue = compileKotodamaProgram(`
seiyaku NamedUnitReturnValue {
  fn helper() -> unit {
    return 1;
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const invalidTupleValue = compileKotodamaProgram(`
seiyaku TupleUnitReturnValue {
  fn helper() -> () {
    return 1;
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.deepEqual(namedUnit.diagnostics, []);
  assert.deepEqual(tupleUnit.diagnostics, []);
  assert.equal(invalidNamedValue.artifactBytes.length, 0);
  assert.equal(invalidNamedValue.diagnostics.length, 1);
  assert.match(invalidNamedValue.diagnostics[0].message, /return type mismatch: unexpected value/);
  assert.equal(invalidTupleValue.artifactBytes.length, 0);
  assert.equal(invalidTupleValue.diagnostics.length, 1);
  assert.match(
    invalidTupleValue.diagnostics[0].message,
    /return type mismatch: type annotation mismatch: expected \(\), got int/,
  );
});

test("Kotodama compiler SDK allows trigger payload helpers in trigger callbacks", () => {
  const compiled = compileKotodamaProgram(`
seiyaku PayloadCallback {
  kotoage fn run() {
    let event = trigger_event();
    let kind = event.get_name(name("kind"));
    let payload = json!{ amount: 7, "meta": "ok", owner: "alice@wonderland", asset: "rose#wonderland", nft: "n0$wonderland.universal", proof: "010203" };
    let amount = payload.get_int(name("amount"));
    let numeric = payload.get_numeric(name("amount"));
    let meta = payload.get_json(name("meta"));
    let owner = payload.get_account_id(name("owner"));
    let asset = payload.get_asset_definition_id(name("asset"));
    let nft = payload.get_nft_id(name("nft"));
    let proof = payload.get_blob_hex(name("proof"));
    info(amount);
    info(numeric);
    info(tlv_len(pointer_to_norito(kind)));
    info(tlv_len(pointer_to_norito(owner)));
    info(tlv_len(pointer_to_norito(asset)));
    info(tlv_len(pointer_to_norito(nft)));
    info(tlv_len(proof));
  }

  register_trigger wake {
    call run;
    on pipeline block;
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(compiled.manifest?.entrypoints[0]?.triggers.length, 1);
});

test("Kotodama compiler SDK encodes RWA data trigger filters", () => {
  const valid = compileKotodamaProgram(`
seiyaku TriggerRwa {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data rwa frozen {
      rwa "0707070707070707070707070707070707070707070707070707070707070707$wonderland.universal";
    }
  }
}
`);
  const invalid = compileKotodamaProgram(`
seiyaku InvalidTriggerRwa {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data rwa frozen {
      rwa "0000$wonderland.universal";
    }
  }
}
`);

  assert.deepEqual(valid.diagnostics, []);
  assert.equal(valid.manifest?.entrypoints[0]?.triggers.length, 1);
  assert.equal(invalid.artifactBytes.length, 0);
  assert.equal(invalid.diagnostics.length, 1);
  assert.match(invalid.diagnostics[0].message, /trigger `wake` has invalid `rwa` matcher literal `0000\$wonderland\.universal` in `rwa` data filter/);
  assert.equal(invalid.diagnostics[0].line, 8);
});

test("Kotodama compiler SDK mirrors upstream pipeline trigger filters", () => {
  const block = compileKotodamaProgram(`
seiyaku TriggerBlock {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on pipeline block approved;
  }
}
`);
  const transaction = compileKotodamaProgram(`
seiyaku TriggerTransaction {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on pipeline transaction approved;
  }
}
`);
  const invalid = compileKotodamaProgram(`
seiyaku TriggerMerge {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on pipeline merge;
  }
}
`);

  const blockCntr = noritoPayloadBody(readArtifactSection(block.artifactBytes, 17).payload);
  const transactionCntr = noritoPayloadBody(readArtifactSection(transaction.artifactBytes, 17).payload);

  assert.deepEqual(block.diagnostics, []);
  assert.deepEqual(transaction.diagnostics, []);
  assert.notEqual(blockCntr.indexOf(Buffer.from([
    0x01, 0x00, 0x00, 0x00,
    0x09,
    0x01, 0x00,
    0x06, 0x01, 0x04, 0x01, 0x00, 0x00, 0x00,
  ])), -1);
  assert.notEqual(transactionCntr.indexOf(Buffer.from([
    0x00, 0x00, 0x00, 0x00,
    0x0f,
    0x01, 0x00,
    0x01, 0x00,
    0x01, 0x00,
    0x01, 0x00,
    0x06, 0x01, 0x04, 0x02, 0x00, 0x00, 0x00,
  ])), -1);
  assert.equal(invalid.artifactBytes.length, 0);
  assert.equal(invalid.diagnostics.length, 1);
  assert.match(invalid.diagnostics[0].message, /transaction \[approved\]/);
});

test("Kotodama compiler SDK accepts encoded account trigger authority and matchers", () => {
  const publicKey = "ed0120aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
  const account = renderCanonicalAccountIdLiteralFromPublicKeyLiteral(publicKey);
  assert.ok(account);

  const compiled = compileKotodamaProgram(`
seiyaku EncodedAccountTrigger {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data account metadata_inserted {
      account "${account}";
    }
    authority "${account}";
  }
}
`);
  const invalid = compileKotodamaProgram(`
seiyaku InvalidEncodedAccountTrigger {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data account metadata_inserted {
      account "alice@wonderland";
    }
    authority "alice@wonderland";
  }
}
`);
  const invalidPublicKeyMatcher = compileKotodamaProgram(`
seiyaku InvalidPublicKeyAccountMatcher {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data account metadata_inserted {
      account "${publicKey}";
    }
  }
}
`);
  const invalidPublicKeyAuthority = compileKotodamaProgram(`
seiyaku InvalidPublicKeyTriggerAuthority {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data any;
    authority "${publicKey}";
  }
}
`);
  const cntrBody = noritoPayloadBody(readArtifactSection(compiled.artifactBytes, 17).payload);
  const compactPublicKeyNeedle = Buffer.from([
    0x21, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x00,
    ...Array.from({ length: 32 }, () => [0x01, 0xaa]).flat(),
  ]);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(compiled.manifest?.entrypoints[0]?.triggers.length, 1);
  assert.notEqual(cntrBody.indexOf(compactPublicKeyNeedle), -1);
  assert.equal(invalid.artifactBytes.length, 0);
  assert.equal(invalid.diagnostics.length, 1);
  assert.match(
    invalid.diagnostics[0].message,
    /trigger `wake` has invalid `account` matcher literal `alice@wonderland` in `account` data filter: AccountId must use a canonical I105 literal/,
  );
  assert.equal(invalid.diagnostics[0].line, 8);
  assert.equal(invalidPublicKeyMatcher.artifactBytes.length, 0);
  assert.equal(invalidPublicKeyMatcher.diagnostics.length, 1);
  assert.match(
    invalidPublicKeyMatcher.diagnostics[0].message,
    /trigger `wake` has invalid `account` matcher literal `ed0120aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa` in `account` data filter: AccountId must use a canonical I105 literal/,
  );
  assert.equal(invalidPublicKeyMatcher.diagnostics[0].line, 8);
  assert.equal(invalidPublicKeyAuthority.artifactBytes.length, 0);
  assert.equal(invalidPublicKeyAuthority.diagnostics.length, 1);
  assert.match(
    invalidPublicKeyAuthority.diagnostics[0].message,
    /semantic error: invalid trigger authority `ed0120aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`: AccountId must use a canonical I105 literal/,
  );
  assert.equal(invalidPublicKeyAuthority.diagnostics[0].line, 8);
});

test("Kotodama compiler SDK rejects duplicate trigger control fields like Rust", () => {
  const publicKey = "ed0120aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
  const account = renderCanonicalAccountIdLiteralFromPublicKeyLiteral(publicKey);
  assert.ok(account);

  const duplicateCall = compileKotodamaProgram(`
seiyaku DuplicateTriggerCall {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    call run;
    on time pre_commit;
  }
}
`);
  const duplicateOn = compileKotodamaProgram(`
seiyaku DuplicateTriggerOn {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time pre_commit;
    on time pre_commit;
  }
}
`);
  const duplicateRepeats = compileKotodamaProgram(`
seiyaku DuplicateTriggerRepeats {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time pre_commit;
    repeats 1;
    repeats 2;
  }
}
`);
  const duplicateAuthority = compileKotodamaProgram(`
seiyaku DuplicateTriggerAuthority {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time pre_commit;
    authority "${account}";
    authority "${account}";
  }
}
`);

  assert.equal(duplicateCall.artifactBytes.length, 0);
  assert.equal(duplicateCall.diagnostics.length, 1);
  assert.match(duplicateCall.diagnostics[0].message, /parser error: \{error\}: expected duplicate `call` field but found Call/);
  assert.equal(duplicateCall.diagnostics[0].line, 7);
  assert.equal(duplicateOn.artifactBytes.length, 0);
  assert.equal(duplicateOn.diagnostics.length, 1);
  assert.match(duplicateOn.diagnostics[0].message, /parser error: \{error\}: expected duplicate `on` field but found Ident\("on"\)/);
  assert.equal(duplicateOn.diagnostics[0].line, 8);
  assert.equal(duplicateRepeats.artifactBytes.length, 0);
  assert.equal(duplicateRepeats.diagnostics.length, 1);
  assert.match(duplicateRepeats.diagnostics[0].message, /parser error: \{error\}: expected duplicate `repeats` field but found Ident\("repeats"\)/);
  assert.equal(duplicateRepeats.diagnostics[0].line, 9);
  assert.equal(duplicateAuthority.artifactBytes.length, 0);
  assert.equal(duplicateAuthority.diagnostics.length, 1);
  assert.match(duplicateAuthority.diagnostics[0].message, /parser error: \{error\}: expected duplicate `authority` field but found Ident\("authority"\)/);
  assert.equal(duplicateAuthority.diagnostics[0].line, 9);
});

test("Kotodama compiler SDK reports missing and unknown trigger fields like Rust", () => {
  const missingCall = compileKotodamaProgram(`
seiyaku MissingTriggerCall {
  kotoage fn run() {}

  register_trigger wake {
    on time pre_commit;
  }
}
`);
  const missingOn = compileKotodamaProgram(`
seiyaku MissingTriggerOn {
  kotoage fn run() {}

  register_trigger wake {
    call run;
  }
}
`);
  const unknownField = compileKotodamaProgram(`
seiyaku UnknownTriggerField {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time pre_commit;
    enabled true;
  }
}
`);

  assert.equal(missingCall.artifactBytes.length, 0);
  assert.equal(missingCall.diagnostics.length, 1);
  assert.match(
    missingCall.diagnostics[0].message,
    /parser error: \{error\}: expected trigger `call` field but found Ident\("register_trigger"\)/,
  );
  assert.equal(missingCall.diagnostics[0].line, 5);
  assert.equal(missingOn.artifactBytes.length, 0);
  assert.equal(missingOn.diagnostics.length, 1);
  assert.match(
    missingOn.diagnostics[0].message,
    /parser error: \{error\}: expected trigger `on` field but found Ident\("register_trigger"\)/,
  );
  assert.equal(missingOn.diagnostics[0].line, 5);
  assert.equal(unknownField.artifactBytes.length, 0);
  assert.equal(unknownField.diagnostics.length, 1);
  assert.match(
    unknownField.diagnostics[0].message,
    /parser error: \{error\}: expected trigger field \(`call`, `on`, `repeats`, `authority`, `metadata`\) but found Ident\("enabled"\)/,
  );
  assert.equal(unknownField.diagnostics[0].line, 8);
});

test("Kotodama compiler SDK reports invalid trigger repeats like Rust", () => {
  const negative = compileKotodamaProgram(`
seiyaku RepeatsNegative {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time pre_commit;
    repeats -1;
  }
}
`);
  const overflow = compileKotodamaProgram(`
seiyaku RepeatsOverflow {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time pre_commit;
    repeats 4294967296;
  }
}
`);
  const string = compileKotodamaProgram(`
seiyaku RepeatsString {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time pre_commit;
    repeats "two";
  }
}
`);

  assert.equal(negative.artifactBytes.length, 0);
  assert.equal(negative.diagnostics.length, 1);
  assert.match(
    negative.diagnostics[0].message,
    /parser error: \{error\}: expected repeats expects a non-negative integer literal but found Minus/,
  );
  assert.equal(negative.diagnostics[0].line, 8);
  assert.equal(negative.diagnostics[0].column, 13);
  assert.equal(overflow.artifactBytes.length, 0);
  assert.equal(overflow.diagnostics.length, 1);
  assert.match(overflow.diagnostics[0].message, /parser error: repeats integer literal out of range/);
  assert.equal(overflow.diagnostics[0].line, 8);
  assert.equal(overflow.diagnostics[0].column, 13);
  assert.equal(string.artifactBytes.length, 0);
  assert.equal(string.diagnostics.length, 1);
  assert.match(
    string.diagnostics[0].message,
    /parser error: \{error\}: expected repeats expects a non-negative integer literal but found String\("two"\)/,
  );
  assert.equal(string.diagnostics[0].line, 8);
  assert.equal(string.diagnostics[0].column, 13);
});

test("Kotodama compiler SDK reports invalid trigger schedules like Rust", () => {
  const startNegative = compileKotodamaProgram(`
seiyaku ScheduleStartNegative {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time schedule(-1, 1);
  }
}
`);
  const periodNegative = compileKotodamaProgram(`
seiyaku SchedulePeriodNegative {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time schedule(1, -1);
  }
}
`);
  const startString = compileKotodamaProgram(`
seiyaku ScheduleStartString {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time schedule("now", 1);
  }
}
`);
  const periodString = compileKotodamaProgram(`
seiyaku SchedulePeriodString {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time schedule(1, "later");
  }
}
`);
  const startDecimal = compileKotodamaProgram(`
seiyaku ScheduleStartDecimal {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time schedule(1.2, 1);
  }
}
`);
  const u64Max = compileKotodamaProgram(`
seiyaku ScheduleU64Max {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time schedule(18446744073709551615, 18446744073709551615);
  }
}
`);
  const u64Overflow = compileKotodamaProgram(`
seiyaku ScheduleU64Overflow {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time schedule(18446744073709551616, 1);
  }
}
`);

  assert.equal(startNegative.artifactBytes.length, 0);
  assert.equal(startNegative.diagnostics.length, 1);
  assert.match(
    startNegative.diagnostics[0].message,
    /parser error: \{error\}: expected schedule start_ms expects a non-negative integer literal but found Minus/,
  );
  assert.equal(startNegative.diagnostics[0].line, 7);
  assert.equal(startNegative.diagnostics[0].column, 22);
  assert.equal(periodNegative.artifactBytes.length, 0);
  assert.equal(periodNegative.diagnostics.length, 1);
  assert.match(
    periodNegative.diagnostics[0].message,
    /parser error: \{error\}: expected schedule period_ms expects a non-negative integer literal but found Minus/,
  );
  assert.equal(periodNegative.diagnostics[0].line, 7);
  assert.equal(periodNegative.diagnostics[0].column, 25);
  assert.equal(startString.artifactBytes.length, 0);
  assert.equal(startString.diagnostics.length, 1);
  assert.match(
    startString.diagnostics[0].message,
    /parser error: \{error\}: expected schedule start_ms expects a non-negative integer literal but found String\("now"\)/,
  );
  assert.equal(startString.diagnostics[0].line, 7);
  assert.equal(startString.diagnostics[0].column, 22);
  assert.equal(periodString.artifactBytes.length, 0);
  assert.equal(periodString.diagnostics.length, 1);
  assert.match(
    periodString.diagnostics[0].message,
    /parser error: \{error\}: expected schedule period_ms expects a non-negative integer literal but found String\("later"\)/,
  );
  assert.equal(periodString.diagnostics[0].line, 7);
  assert.equal(periodString.diagnostics[0].column, 25);
  assert.equal(startDecimal.artifactBytes.length, 0);
  assert.equal(startDecimal.diagnostics.length, 1);
  assert.match(
    startDecimal.diagnostics[0].message,
    /parser error: \{error\}: expected schedule start_ms expects a non-negative integer literal but found Decimal\("1.2"\)/,
  );
  assert.equal(startDecimal.diagnostics[0].line, 7);
  assert.equal(startDecimal.diagnostics[0].column, 22);
  assert.deepEqual(u64Max.diagnostics, []);
  assert.notEqual(u64Max.artifactBytes.length, 0);
  assert.equal(u64Overflow.artifactBytes.length, 0);
  assert.equal(u64Overflow.diagnostics.length, 1);
  assert.match(u64Overflow.diagnostics[0].message, /numeric literal overflow at 7:22/);
  assert.equal(u64Overflow.diagnostics[0].line, 7);
  assert.equal(u64Overflow.diagnostics[0].column, 22);
});

test("Kotodama compiler SDK reports invalid trigger filter forms like Rust", () => {
  const unknownFilter = compileKotodamaProgram(`seiyaku UnknownFilter {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on oracle tick;
  }
}
`);
  const executeWrongKind = compileKotodamaProgram(`seiyaku ExecuteWrongKind {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on execute account "wake";
  }
}
`);
  const timeWrongKind = compileKotodamaProgram(`seiyaku TimeWrongKind {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time post_commit;
  }
}
`);
  const pipelineWrongKind = compileKotodamaProgram(`seiyaku PipelineWrongKind {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on pipeline epoch;
  }
}
`);
  const dataWrongFamily = compileKotodamaProgram(`seiyaku DataWrongFamily {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data wallet created {}
  }
}
`);

  assert.equal(unknownFilter.artifactBytes.length, 0);
  assert.equal(unknownFilter.diagnostics.length, 1);
  assert.match(
    unknownFilter.diagnostics[0].message,
    /parser error: \{error\}: expected trigger filter \(`time`, `execute`, `data`, or `pipeline`\) but found Ident\("oracle"\)/,
  );
  assert.equal(unknownFilter.diagnostics[0].line, 6);
  assert.equal(unknownFilter.diagnostics[0].column, 8);
  assert.equal(executeWrongKind.artifactBytes.length, 0);
  assert.equal(executeWrongKind.diagnostics.length, 1);
  assert.match(
    executeWrongKind.diagnostics[0].message,
    /parser error: \{error\}: expected execute trigger <name> but found Ident\("account"\)/,
  );
  assert.equal(executeWrongKind.diagnostics[0].line, 6);
  assert.equal(executeWrongKind.diagnostics[0].column, 16);
  assert.equal(timeWrongKind.artifactBytes.length, 0);
  assert.equal(timeWrongKind.diagnostics.length, 1);
  assert.match(
    timeWrongKind.diagnostics[0].message,
    /parser error: \{error\}: expected time filter \(`pre_commit` or `schedule`\) but found Ident\("post_commit"\)/,
  );
  assert.equal(timeWrongKind.diagnostics[0].line, 6);
  assert.equal(timeWrongKind.diagnostics[0].column, 13);
  assert.equal(pipelineWrongKind.artifactBytes.length, 0);
  assert.equal(pipelineWrongKind.diagnostics.length, 1);
  assert.match(
    pipelineWrongKind.diagnostics[0].message,
    /parser error: \{error\}: expected pipeline filter \(`transaction \[approved\]` or `block \[approved\]`\) but found Ident\("epoch"\)/,
  );
  assert.equal(pipelineWrongKind.diagnostics[0].line, 6);
  assert.equal(pipelineWrongKind.diagnostics[0].column, 17);
  assert.equal(dataWrongFamily.artifactBytes.length, 0);
  assert.equal(dataWrongFamily.diagnostics.length, 1);
  assert.match(
    dataWrongFamily.diagnostics[0].message,
    /parser error: \{error\}: expected data family \(`any`, `peer`, `domain`, `account`, `asset`, `asset_definition`, `nft`, `rwa`, `trigger`, `role`, `configuration`, or `executor`\) but found Ident\("wallet"\)/,
  );
  assert.equal(dataWrongFamily.diagnostics[0].line, 6);
  assert.equal(dataWrongFamily.diagnostics[0].column, 13);
});

test("Kotodama compiler SDK reports invalid trigger identifier operands like Rust", () => {
  const executeIdNumber = compileKotodamaProgram(`seiyaku ExecuteIdNumber {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on execute trigger 123;
  }
}
`);
  const authorityNumber = compileKotodamaProgram(`seiyaku AuthorityNumber {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time pre_commit;
    authority 123;
  }
}
`);
  const matcherValueNumber = compileKotodamaProgram(`seiyaku MatcherValueNumber {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data account created {
      account 123;
    }
  }
}
`);
  const matcherKeyString = compileKotodamaProgram(`seiyaku MatcherKeyString {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data account created {
      "account" "alice";
    }
  }
}
`);
  const metadataKeyNumber = compileKotodamaProgram(`seiyaku MetadataKeyNumber {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time pre_commit;
    metadata {
      1: "x";
    }
  }
}
`);

  assert.equal(executeIdNumber.artifactBytes.length, 0);
  assert.equal(executeIdNumber.diagnostics.length, 1);
  assert.match(
    executeIdNumber.diagnostics[0].message,
    /parser error: \{error\}: expected identifier or string literal but found Number\(123\)/,
  );
  assert.equal(executeIdNumber.diagnostics[0].line, 6);
  assert.equal(executeIdNumber.diagnostics[0].column, 24);
  assert.equal(authorityNumber.artifactBytes.length, 0);
  assert.equal(authorityNumber.diagnostics.length, 1);
  assert.match(
    authorityNumber.diagnostics[0].message,
    /parser error: \{error\}: expected identifier or string literal but found Number\(123\)/,
  );
  assert.equal(authorityNumber.diagnostics[0].line, 7);
  assert.equal(authorityNumber.diagnostics[0].column, 15);
  assert.equal(matcherValueNumber.artifactBytes.length, 0);
  assert.equal(matcherValueNumber.diagnostics.length, 1);
  assert.match(
    matcherValueNumber.diagnostics[0].message,
    /parser error: \{error\}: expected identifier or string literal but found Number\(123\)/,
  );
  assert.equal(matcherValueNumber.diagnostics[0].line, 7);
  assert.equal(matcherValueNumber.diagnostics[0].column, 15);
  assert.equal(matcherKeyString.artifactBytes.length, 0);
  assert.equal(matcherKeyString.diagnostics.length, 1);
  assert.match(
    matcherKeyString.diagnostics[0].message,
    /parser error: \{error\}: expected identifier but found String\("account"\)/,
  );
  assert.equal(matcherKeyString.diagnostics[0].line, 7);
  assert.equal(matcherKeyString.diagnostics[0].column, 7);
  assert.equal(metadataKeyNumber.artifactBytes.length, 0);
  assert.equal(metadataKeyNumber.diagnostics.length, 1);
  assert.match(
    metadataKeyNumber.diagnostics[0].message,
    /parser error: \{error\}: expected metadata key \(identifier or string literal\) but found Number\(1\)/,
  );
  assert.equal(metadataKeyNumber.diagnostics[0].line, 8);
  assert.equal(metadataKeyNumber.diagnostics[0].column, 7);
});

test("Kotodama compiler SDK reports malformed trigger punctuation like Rust", () => {
  const callMissingSemicolon = compileKotodamaProgram(`seiyaku CallMissingSemicolon {
  kotoage fn run() {}

  register_trigger wake {
    call run
    on time pre_commit;
  }
}
`);
  const matcherMissingSemicolon = compileKotodamaProgram(`seiyaku MatcherMissingSemicolon {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data account created {
      account "alice"
    }
  }
}
`);
  const metadataMissingColon = compileKotodamaProgram(`seiyaku MetadataMissingColon {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time pre_commit;
    metadata {
      tag "x";
    }
  }
}
`);
  const metadataMissingSemicolon = compileKotodamaProgram(`seiyaku MetadataMissingSemicolon {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time pre_commit;
    metadata {
      tag: "x"
    }
  }
}
`);
  const dataMissingBlock = compileKotodamaProgram(`seiyaku DataMissingBlock {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data account created;
  }
}
`);
  const metadataNoBlock = compileKotodamaProgram(`seiyaku MetadataNoBlock {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time pre_commit;
    metadata;
  }
}
`);
  const scheduleNoParen = compileKotodamaProgram(`seiyaku ScheduleNoParen {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time schedule;
  }
}
`);
  const scheduleMissingComma = compileKotodamaProgram(`seiyaku ScheduleMissingComma {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time schedule(1 2);
  }
}
`);
  const scheduleMissingRParen = compileKotodamaProgram(`seiyaku ScheduleMissingRParen {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time schedule(1, 2;
  }
}
`);
  const metadataValueComma = compileKotodamaProgram(`seiyaku MetadataValueComma {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time pre_commit;
    metadata {
      tag: "x",
    }
  }
}
`);

  assert.equal(callMissingSemicolon.artifactBytes.length, 0);
  assert.equal(callMissingSemicolon.diagnostics.length, 1);
  assert.match(callMissingSemicolon.diagnostics[0].message, /parser error: \{error\}: expected Semicolon but found Ident\("on"\)/);
  assert.equal(callMissingSemicolon.diagnostics[0].line, 6);
  assert.equal(callMissingSemicolon.diagnostics[0].column, 5);
  assert.equal(matcherMissingSemicolon.artifactBytes.length, 0);
  assert.equal(matcherMissingSemicolon.diagnostics.length, 1);
  assert.match(matcherMissingSemicolon.diagnostics[0].message, /parser error: \{error\}: expected Semicolon but found RBrace/);
  assert.equal(matcherMissingSemicolon.diagnostics[0].line, 8);
  assert.equal(matcherMissingSemicolon.diagnostics[0].column, 5);
  assert.equal(metadataMissingColon.artifactBytes.length, 0);
  assert.equal(metadataMissingColon.diagnostics.length, 1);
  assert.match(metadataMissingColon.diagnostics[0].message, /parser error: \{error\}: expected Colon but found String\("x"\)/);
  assert.equal(metadataMissingColon.diagnostics[0].line, 8);
  assert.equal(metadataMissingColon.diagnostics[0].column, 11);
  assert.equal(metadataMissingSemicolon.artifactBytes.length, 0);
  assert.equal(metadataMissingSemicolon.diagnostics.length, 1);
  assert.match(metadataMissingSemicolon.diagnostics[0].message, /parser error: \{error\}: expected Semicolon but found RBrace/);
  assert.equal(metadataMissingSemicolon.diagnostics[0].line, 9);
  assert.equal(metadataMissingSemicolon.diagnostics[0].column, 5);
  assert.equal(dataMissingBlock.artifactBytes.length, 0);
  assert.equal(dataMissingBlock.diagnostics.length, 1);
  assert.match(dataMissingBlock.diagnostics[0].message, /parser error: \{error\}: expected LBrace but found Semicolon/);
  assert.equal(dataMissingBlock.diagnostics[0].line, 6);
  assert.equal(dataMissingBlock.diagnostics[0].column, 28);
  assert.equal(metadataNoBlock.artifactBytes.length, 0);
  assert.equal(metadataNoBlock.diagnostics.length, 1);
  assert.match(metadataNoBlock.diagnostics[0].message, /parser error: \{error\}: expected LBrace but found Semicolon/);
  assert.equal(metadataNoBlock.diagnostics[0].line, 7);
  assert.equal(metadataNoBlock.diagnostics[0].column, 13);
  assert.equal(scheduleNoParen.artifactBytes.length, 0);
  assert.equal(scheduleNoParen.diagnostics.length, 1);
  assert.match(scheduleNoParen.diagnostics[0].message, /parser error: \{error\}: expected LParen but found Semicolon/);
  assert.equal(scheduleNoParen.diagnostics[0].line, 6);
  assert.equal(scheduleNoParen.diagnostics[0].column, 21);
  assert.equal(scheduleMissingComma.artifactBytes.length, 0);
  assert.equal(scheduleMissingComma.diagnostics.length, 1);
  assert.match(scheduleMissingComma.diagnostics[0].message, /parser error: \{error\}: expected RParen but found Number\(2\)/);
  assert.equal(scheduleMissingComma.diagnostics[0].line, 6);
  assert.equal(scheduleMissingComma.diagnostics[0].column, 24);
  assert.equal(scheduleMissingRParen.artifactBytes.length, 0);
  assert.equal(scheduleMissingRParen.diagnostics.length, 1);
  assert.match(scheduleMissingRParen.diagnostics[0].message, /parser error: \{error\}: expected RParen but found Semicolon/);
  assert.equal(scheduleMissingRParen.diagnostics[0].line, 6);
  assert.equal(scheduleMissingRParen.diagnostics[0].column, 26);
  assert.equal(metadataValueComma.artifactBytes.length, 0);
  assert.equal(metadataValueComma.diagnostics.length, 1);
  assert.match(metadataValueComma.diagnostics[0].message, /parser error: \{error\}: expected Semicolon but found Comma/);
  assert.equal(metadataValueComma.diagnostics[0].line, 8);
  assert.equal(metadataValueComma.diagnostics[0].column, 15);
});

test("Kotodama compiler SDK replaces earlier duplicate trigger metadata blocks like Rust", () => {
  const compiled = compileKotodamaProgram(`
seiyaku DuplicateTriggerMetadata {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time pre_commit;
    metadata {
      tag: "first";
    }
    metadata {
      tag: "second";
    }
  }
}
`);
  const artifact = Buffer.from(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(compiled.artifactBytes.length, 0);
  assert.equal(artifact.indexOf("first"), -1);
  assert.notEqual(artifact.indexOf("second"), -1);
});

test("Kotodama compiler SDK mirrors Rust trigger metadata semantic diagnostics", () => {
  const compileMetadata = (body) => compileKotodamaProgram(`
seiyaku InvalidTriggerMetadata {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on time pre_commit;
    metadata {
      ${body}
    }
  }
}
`);
  const emptyKey = compileMetadata('"": 1;');
  const whitespaceKey = compileMetadata('" tag": 1;');
  const reservedKey = compileMetadata('"bad$key": 1;');
  const duplicateKey = compileMetadata('tag: "a"; tag: "b";');
  const jsonCallNumber = compileMetadata('tag: json(1);');
  const nameCallValue = compileMetadata('tag: name("x");');

  assert.equal(emptyKey.artifactBytes.length, 0);
  assert.equal(emptyKey.diagnostics[0].message, "semantic error: invalid trigger metadata key ``: Empty `Name`");
  assert.equal(whitespaceKey.artifactBytes.length, 0);
  assert.equal(whitespaceKey.diagnostics[0].message, "semantic error: invalid trigger metadata key ` tag`: White space not allowed in `Name` constructs");
  assert.equal(reservedKey.artifactBytes.length, 0);
  assert.equal(reservedKey.diagnostics[0].message, "semantic error: invalid trigger metadata key `bad$key`: The `@` character is reserved for scoped alias/public-key constructs, `#` for alias separators (for example `name#domain.dataspace`), and `$` — for `nft$domain`.");
  assert.equal(duplicateKey.artifactBytes.length, 0);
  assert.equal(duplicateKey.diagnostics[0].message, "semantic error: duplicate trigger metadata key `tag`");
  assert.equal(jsonCallNumber.artifactBytes.length, 0);
  assert.equal(jsonCallNumber.diagnostics[0].message, "semantic error: json(...) metadata values must be a string literal");
  assert.equal(nameCallValue.artifactBytes.length, 0);
  assert.equal(nameCallValue.diagnostics[0].message, "semantic error: trigger metadata values must be JSON literals");
});

test("Kotodama compiler SDK rejects duplicate data trigger matchers like Rust", () => {
  const compiled = compileKotodamaProgram(`
seiyaku DuplicateMatcher {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data asset added {
      asset_definition "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
      asset_definition "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    }
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(
    compiled.diagnostics[0].message,
    /trigger `wake` has duplicate `asset_definition` matcher in `asset` data filter/,
  );
  assert.equal(compiled.diagnostics[0].line, 9);
});

test("Kotodama compiler SDK reports invalid data trigger matcher literals like Rust", () => {
  const compiled = compileKotodamaProgram(`
seiyaku InvalidMatcherLiteral {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data asset added {
      asset_definition "not-an-asset-def";
    }
  }
}
`);
  const invalidPeerBeforeDuplicate = compileKotodamaProgram(`
seiyaku InvalidPeerMatcherBeforeDuplicate {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data peer added {
      peer "ed0120aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
      peer "ed0120aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    }
  }
}
`);
  const invalidTriggerName = compileKotodamaProgram(`
seiyaku InvalidTriggerMatcherLiteral {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data trigger created {
      trigger "not a trigger";
    }
  }
}
`);
  const invalidRoleName = compileKotodamaProgram(`
seiyaku InvalidRoleMatcherLiteral {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data role created {
      role "not a role";
    }
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(
    compiled.diagnostics[0].message,
    /trigger `wake` has invalid `asset_definition` matcher literal `not-an-asset-def` in `asset` data filter: Asset Definition ID must be valid Base58/,
  );
  assert.equal(compiled.diagnostics[0].line, 8);
  assert.equal(invalidPeerBeforeDuplicate.artifactBytes.length, 0);
  assert.equal(invalidPeerBeforeDuplicate.diagnostics.length, 1);
  assert.match(
    invalidPeerBeforeDuplicate.diagnostics[0].message,
    /trigger `wake` has invalid `peer` matcher literal `ed0120aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa` in `peer` data filter: Non-canonical multihash hex/,
  );
  assert.equal(invalidPeerBeforeDuplicate.diagnostics[0].line, 8);
  assert.equal(invalidTriggerName.artifactBytes.length, 0);
  assert.equal(invalidTriggerName.diagnostics.length, 1);
  assert.match(
    invalidTriggerName.diagnostics[0].message,
    /trigger `wake` has invalid `trigger` matcher literal `not a trigger` in `trigger` data filter: White space not allowed in `Name` constructs/,
  );
  assert.equal(invalidTriggerName.diagnostics[0].line, 8);
  assert.equal(invalidRoleName.artifactBytes.length, 0);
  assert.equal(invalidRoleName.diagnostics.length, 1);
  assert.match(
    invalidRoleName.diagnostics[0].message,
    /trigger `wake` has invalid `role` matcher literal `not a role` in `role` data filter: White space not allowed in `Name` constructs/,
  );
  assert.equal(invalidRoleName.diagnostics[0].line, 8);
});

test("Kotodama compiler SDK encodes upstream account trigger event-set bits", () => {
  const publicKey = "ed0120bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
  const account = renderCanonicalAccountIdLiteralFromPublicKeyLiteral(publicKey);
  assert.ok(account);

  const repo = compileKotodamaProgram(`
seiyaku AccountRepoTrigger {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data account repo {
      account "${account}";
    }
  }
}
`);
  const metadata = compileKotodamaProgram(`
seiyaku AccountMetadataTrigger {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data account metadata_inserted {
      account "${account}";
    }
  }
}
`);
  const any = compileKotodamaProgram(`
seiyaku AccountAnyTrigger {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data account any {
      account "${account}";
    }
  }
}
`);
  const invalid = compileKotodamaProgram(`
seiyaku AccountRecoveryTrigger {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data account recovery {}
  }
}
`);

  const repoCntr = noritoPayloadBody(readArtifactSection(repo.artifactBytes, 17).payload);
  const metadataCntr = noritoPayloadBody(readArtifactSection(metadata.artifactBytes, 17).payload);
  const anyCntr = noritoPayloadBody(readArtifactSection(any.artifactBytes, 17).payload);

  assert.deepEqual(repo.diagnostics, []);
  assert.deepEqual(metadata.diagnostics, []);
  assert.deepEqual(any.diagnostics, []);
  assert.notEqual(repoCntr.indexOf(compactBitmapU32Needle(0x0000_0800)), -1);
  assert.notEqual(metadataCntr.indexOf(compactBitmapU32Needle(0x0000_0100)), -1);
  assert.notEqual(anyCntr.indexOf(compactBitmapU32Needle(0x0000_0fff)), -1);
  assert.equal(invalid.artifactBytes.length, 0);
  assert.equal(invalid.diagnostics.length, 1);
  assert.match(invalid.diagnostics[0].message, /trigger `wake` does not support `recovery` event kind for `account` data filter/);
  assert.equal(invalid.diagnostics[0].line, 7);
});

test("Kotodama compiler SDK encodes upstream domain trigger event-set bits", () => {
  const account = compileKotodamaProgram(`
seiyaku DomainAccountTrigger {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data domain account {
      domain "wonderland.universal";
    }
  }
}
`);
  const linked = compileKotodamaProgram(`
seiyaku DomainLinkedTrigger {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data domain account_linked {
      domain "wonderland.universal";
    }
  }
}
`);
  const streaming = compileKotodamaProgram(`
seiyaku DomainStreamingTrigger {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data domain streaming_ticket_revoked {
      domain "wonderland.universal";
    }
  }
}
`);
  const any = compileKotodamaProgram(`
seiyaku DomainAnyTrigger {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data domain any {
      domain "wonderland.universal";
    }
  }
}
`);
  const invalid = compileKotodamaProgram(`
seiyaku DomainRwaTrigger {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data domain rwa {}
  }
}
`);

  const accountCntr = noritoPayloadBody(readArtifactSection(account.artifactBytes, 17).payload);
  const linkedCntr = noritoPayloadBody(readArtifactSection(linked.artifactBytes, 17).payload);
  const streamingCntr = noritoPayloadBody(readArtifactSection(streaming.artifactBytes, 17).payload);
  const anyCntr = noritoPayloadBody(readArtifactSection(any.artifactBytes, 17).payload);

  assert.deepEqual(account.diagnostics, []);
  assert.deepEqual(linked.diagnostics, []);
  assert.deepEqual(streaming.diagnostics, []);
  assert.deepEqual(any.diagnostics, []);
  assert.notEqual(accountCntr.indexOf(compactBitmapU32Needle(0x0000_0020)), -1);
  assert.notEqual(linkedCntr.indexOf(compactBitmapU32Needle(0x0000_0040)), -1);
  assert.notEqual(streamingCntr.indexOf(compactBitmapU32Needle(0x0002_0000)), -1);
  assert.notEqual(anyCntr.indexOf(compactBitmapU32Needle(0x0003_ffff)), -1);
  assert.equal(invalid.artifactBytes.length, 0);
  assert.equal(invalid.diagnostics.length, 1);
  assert.match(invalid.diagnostics[0].message, /trigger `wake` does not support `rwa` event kind for `domain` data filter/);
  assert.equal(invalid.diagnostics[0].line, 7);
});

test("Kotodama compiler SDK encodes upstream asset-definition trigger event-set bits", () => {
  const detailed = compileKotodamaProgram(`
seiyaku AssetDefinitionDetailedTrigger {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data asset_definition mintability_changed_detailed {}
  }
}
`);
  const total = compileKotodamaProgram(`
seiyaku AssetDefinitionTotalTrigger {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data asset_definition total_quantity_changed {}
  }
}
`);
  const owner = compileKotodamaProgram(`
seiyaku AssetDefinitionOwnerTrigger {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data asset_definition owner_changed {}
  }
}
`);
  const any = compileKotodamaProgram(`
seiyaku AssetDefinitionAnyTrigger {
  kotoage fn run() {}

  register_trigger wake {
    call run;
    on data asset_definition any {}
  }
}
`);

  const detailedCntr = noritoPayloadBody(readArtifactSection(detailed.artifactBytes, 17).payload);
  const totalCntr = noritoPayloadBody(readArtifactSection(total.artifactBytes, 17).payload);
  const ownerCntr = noritoPayloadBody(readArtifactSection(owner.artifactBytes, 17).payload);
  const anyCntr = noritoPayloadBody(readArtifactSection(any.artifactBytes, 17).payload);

  assert.deepEqual(detailed.diagnostics, []);
  assert.deepEqual(total.diagnostics, []);
  assert.deepEqual(owner.diagnostics, []);
  assert.deepEqual(any.diagnostics, []);
  assert.notEqual(detailedCntr.indexOf(compactBitmapU32Needle(0x0000_0020)), -1);
  assert.notEqual(totalCntr.indexOf(compactBitmapU32Needle(0x0000_0040)), -1);
  assert.notEqual(ownerCntr.indexOf(compactBitmapU32Needle(0x0000_0080)), -1);
  assert.notEqual(anyCntr.indexOf(compactBitmapU32Needle(0x0000_00ff)), -1);
});

test("Kotodama compiler SDK rejects invalid trigger declarations semantically", () => {
  const duplicate = compileKotodamaProgram(`
seiyaku DuplicateTrigger {
  kotoage fn run() {}

  trigger wake {
    call run;
    on pipeline block;
  }

  register_trigger wake {
    call run;
    on pipeline block;
  }
}
`);

  assert.equal(duplicate.artifactBytes.length, 0);
  assert.equal(duplicate.diagnostics.length, 1);
  assert.match(duplicate.diagnostics[0].message, /duplicate trigger `wake`/);

  const stringName = compileKotodamaProgram(`
seiyaku StringTriggerName {
  kotoage fn run() {}

  register_trigger "wake" {
    call run;
    on time pre_commit;
  }
}
`);

  assert.equal(stringName.artifactBytes.length, 0);
  assert.equal(stringName.diagnostics.length, 1);
  assert.match(stringName.diagnostics[0].message, /parser error: \{error\}: expected identifier but found String\("wake"\)/);
  assert.equal(stringName.diagnostics[0].line, 5);

  const parenthesizedCall = compileKotodamaProgram(`
seiyaku ParenthesizedTriggerCall {
  kotoage fn run() {}

  register_trigger wake {
    call run();
    on time pre_commit;
  }
}
`);

  assert.equal(parenthesizedCall.artifactBytes.length, 0);
  assert.equal(parenthesizedCall.diagnostics.length, 1);
  assert.match(parenthesizedCall.diagnostics[0].message, /parser error: \{error\}: expected Semicolon but found LParen/);
  assert.equal(parenthesizedCall.diagnostics[0].line, 6);

  const zeroPeriod = compileKotodamaProgram(`
seiyaku ZeroPeriodTrigger {
  kotoage fn run() {}

  trigger wake {
    call run;
    on time schedule(0, 0);
  }
}
`);

  assert.equal(zeroPeriod.artifactBytes.length, 0);
  assert.equal(zeroPeriod.diagnostics.length, 1);
  assert.match(zeroPeriod.diagnostics[0].message, /schedule period_ms must be non-zero/);

  const viewTarget = compileKotodamaProgram(`
seiyaku ViewTargetTrigger {
  view fn read() -> int {
    return 1;
  }

  trigger wake {
    call read;
    on pipeline block;
  }
}
`);

  assert.equal(viewTarget.artifactBytes.length, 0);
  assert.equal(viewTarget.diagnostics.length, 1);
  assert.match(viewTarget.diagnostics[0].message, /cannot target read-only view entrypoint `read`/);
});

test("Kotodama compiler SDK derives ZK feature metadata from Poseidon usage", () => {
  const plain = compileKotodamaProgram(`
seiyaku PlainFeature {
  kotoage fn main() -> int {
    return 1;
  }
}
`);
  const poseidon = compileKotodamaProgram(`
seiyaku PoseidonFeature {
  kotoage fn order_id(trader: int, salt: int) -> int {
    let h = poseidon2(trader, salt);
    return h;
  }
}
`);

  assert.deepEqual(plain.diagnostics, []);
  assert.deepEqual(poseidon.diagnostics, []);
  assert.equal(plain.manifest?.features_bitmap, 0);
  assert.equal(plain.artifactBytes[6], 0);
  assert.equal(poseidon.manifest?.features_bitmap, 1);
  assert.equal(poseidon.artifactBytes[6], 1);
});

test("Kotodama compiler SDK emits pubkgen and valcom ZK crypto opcodes", () => {
  const compiled = compileKotodamaProgram(`
seiyaku CommitmentHelpers {
  kotoage fn main() -> int {
    let public_key = pubkgen(7);
    let commitment = valcom(11, 13);
    return public_key + commitment;
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);
  const opcodes = [];
  for (let offset = 0; offset + 3 < code.length; offset += 4) {
    opcodes.push(code[offset + 3]);
  }

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(opcodes.includes(0x84), true);
  assert.equal(opcodes.includes(0x85), true);
  assert.equal(compiled.manifest?.features_bitmap, 1);
  assert.equal(compiled.artifactBytes[6], 1);
});

test("Kotodama compiler SDK mirrors upstream poseidon6 unsupported status", () => {
  const compiled = compileKotodamaProgram(`
seiyaku Poseidon6Unsupported {
  kotoage fn main() -> int {
    let h = poseidon6(1, 2, 3, 4, 5, 6);
    return h;
  }
}
`);
  const invalidArgs = compileKotodamaProgram(`
seiyaku Poseidon6InvalidArgs {
  fn helper() {
    let h = poseidon6(1, 2, 3, 4, 5, name("six"));
  }

  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(compiled.diagnostics[0].message, /POSEIDON6 not supported/);
  assert.equal(invalidArgs.artifactBytes.length, 0);
  assert.equal(invalidArgs.diagnostics.length, 1);
  assert.match(invalidArgs.diagnostics[0].message, /poseidon6 expects six int args/);
});

test("Kotodama compiler SDK emits setvl and derives vector feature metadata", () => {
  const compiled = compileKotodamaProgram(`
seiyaku VectorLength {
  const VL: int = 8;

  kotoage fn main() {
    setvl(VL);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.notEqual(code.indexOf(Buffer.from([0x08, 0x00, 0x00, 0x76])), -1);
  assert.equal(compiled.manifest?.features_bitmap, 2);
  assert.equal(compiled.artifactBytes[6], 2);
});

test("Kotodama compiler SDK derives contract access for inline ZK instruction builders", () => {
  const source = readFileSync(
    new URL("../../../crates/kotodama_lang/src/samples/zk_vote_and_unshield.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "zk_vote_and_unshield.ko" });
  const assetDefinition = "6pEP9RjNoZ7beWkT3pLfKoM1dyfi";
  const account = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB";
  const asset = `${assetDefinition}#${account}`;

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.read_keys, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.write_keys, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.access_hints_complete, true);
  assert.deepEqual(compiled.manifest?.access_set_hints, {
    read_keys: [
      `account:${account}`,
      `asset:${asset}`,
      `asset_def.detail:${assetDefinition}:zk.unshield.last`,
      `asset_def:${assetDefinition}`,
      "zk:election:election-1:ciphertexts",
      "zk:election:election-1:nullifiers",
      `zk_asset:${assetDefinition}`,
    ],
    write_keys: [
      `asset:${asset}`,
      `asset_def.detail:${assetDefinition}:zk.unshield.last`,
      "zk:election:election-1:ciphertexts",
      "zk:election:election-1:nullifiers",
      `zk_asset:${assetDefinition}`,
    ],
    dynamic_reads: [],
    dynamic_writes: [],
  });
});

test("Kotodama compiler SDK keeps ZK vote and unshield rows Rust-shaped", () => {
  const source = readFileSync(
    new URL("../../../crates/kotodama_lang/src/samples/zk_vote_and_unshield.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "zk_vote_and_unshield.ko" });

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ({
      function_name: entry.function_name,
      pc_start: entry.pc_start,
      pc_end: entry.pc_end,
      bytecode_bytes: entry.bytecode_bytes,
      bytecode_words: entry.bytecode_words,
      frame_bytes: entry.frame_bytes,
    })),
    [
      { function_name: "demo", pc_start: 0, pc_end: 1088, bytecode_bytes: 1088, bytecode_words: 272, frame_bytes: 80 },
      { function_name: "verify_and_submit_ballot", pc_start: 1088, pc_end: 1356, bytecode_bytes: 268, bytecode_words: 67, frame_bytes: 40 },
      { function_name: "verify_and_unshield", pc_start: 1356, pc_end: 1832, bytecode_bytes: 476, bytecode_words: 119, frame_bytes: 40 },
    ],
  );
});

test("Kotodama compiler SDK emits inline ZK InstructionBox literals", () => {
  const source = readFileSync(
    new URL("../../../crates/kotodama_lang/src/samples/zk_vote_and_unshield.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "zk_vote_and_unshield.ko" });
  const entries = readLiteralTableEntries(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(
    entries.some((entry) => entry.payload.toString("utf8").includes("kotodama:build_")),
    false,
  );
  assert.equal(entries[2]?.type, 9);
  assert.equal(entries[2]?.length, 522);
  assert.equal(entries[2]?.payload.toString("utf8").startsWith("4e525430"), true);
  assert.equal(entries[4]?.type, 9);
  assert.equal(entries[4]?.length, 261);
  assert.equal(entries[4]?.payload.subarray(0, 4).toString("ascii"), "NRT0");
  assert.equal(entries[5]?.type, 9);
  assert.equal(entries[5]?.length, 832);
  assert.equal(entries[5]?.payload.toString("utf8").startsWith("4e525430"), true);
  assert.equal(entries[7]?.type, 9);
  assert.equal(entries[7]?.length, 416);
  assert.equal(entries[7]?.payload.subarray(0, 4).toString("ascii"), "NRT0");
  assert.equal(entries[12]?.type, 1);
  assert.equal(entries[13]?.type, 2);
});

test("Kotodama compiler SDK treats max_cycles zero as the compiler default", () => {
  const compiled = compileKotodamaProgram(`
seiyaku DefaultCycles {
  meta {
    max_cycles: 0,
  }

  hajimari() {
    info("ready");
  }
}
`);
  const headerCycles = Buffer.from(compiled.artifactBytes.slice(8, 16)).readBigUInt64LE();

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(headerCycles, 1_000_000n);
});

test("Kotodama compiler SDK accepts upstream contract meta aliases", () => {
  const compiled = compileKotodamaProgram(`
seiyaku MetaAliases {
  meta {
    abi: 1,
    cycles: 200000,
    vector: true,
    vl: 8,
  }

  kotoage fn main() {
    setvl(8);
  }
}
`);
  const headerCycles = Buffer.from(compiled.artifactBytes.slice(8, 16)).readBigUInt64LE();

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(headerCycles, 200000n);
  assert.equal(compiled.manifest?.features_bitmap, 2);
  assert.equal(compiled.artifactBytes[6], 2);
  assert.equal(compiled.artifactBytes[7], 8);
});

test("Kotodama compiler SDK mirrors Rust duplicate contract meta key behavior", () => {
  const scalarLastWins = compileKotodamaProgram(`
seiyaku DuplicateScalarMeta {
  meta {
    cycles: 100,
    cycles: 200,
    vl: 4,
    vl: 8,
    vector: false,
    vector: true,
  }

  kotoage fn main() {
    setvl(8);
  }
}
`);
  const multipleMetaBlocks = compileKotodamaProgram(`
seiyaku MultipleMetaBlocks {
  meta {
    cycles: 100,
    features: [],
  }
  meta {
    vl: 8,
    features: [vector],
  }

  kotoage fn main() {
    setvl(8);
  }
}
`);
  const emptyThenVectorFeatures = compileKotodamaProgram(`
seiyaku EmptyThenVectorFeatures {
  meta {
    features: [],
    features: [vector],
  }

  kotoage fn main() {
    setvl(8);
  }
}
`);
  const repeatedNonEmptyFeatures = compileKotodamaProgram(`
seiyaku RepeatedNonEmptyFeatures {
  meta {
    features: [vector],
    features: [],
  }

  kotoage fn main() {
    setvl(8);
  }
}
`);
  const scalarCycles = Buffer.from(scalarLastWins.artifactBytes.slice(8, 16)).readBigUInt64LE();
  const multipleMetaCycles = Buffer.from(multipleMetaBlocks.artifactBytes.slice(8, 16)).readBigUInt64LE();

  assert.deepEqual(scalarLastWins.diagnostics, []);
  assert.equal(scalarCycles, 200n);
  assert.equal(scalarLastWins.artifactBytes[7], 8);
  assert.equal(scalarLastWins.manifest?.features_bitmap, 2);
  assert.deepEqual(multipleMetaBlocks.diagnostics, []);
  assert.equal(multipleMetaCycles, 100n);
  assert.equal(multipleMetaBlocks.artifactBytes[7], 8);
  assert.equal(multipleMetaBlocks.manifest?.features_bitmap, 2);
  assert.deepEqual(emptyThenVectorFeatures.diagnostics, []);
  assert.equal(emptyThenVectorFeatures.manifest?.features_bitmap, 2);
  assert.equal(repeatedNonEmptyFeatures.diagnostics[0].message, "parser error: duplicate meta key 'features'");
});

test("Kotodama compiler SDK mirrors Rust contract meta parser diagnostics", () => {
  const badParamType = compileKotodamaProgram(`
seiyaku ParamNumberType {
  kotoage fn run(value: 123) permission(Admin) {}
}
`);
  const badMetaKey = compileKotodamaProgram(`
seiyaku MetaBadKey {
  meta { 123: 1; }
  kotoage fn run() permission(Admin) {}
}
`);
  const unknownMetaKey = compileKotodamaProgram(`
seiyaku MetaUnknown {
  meta { unknown: 1; }
  kotoage fn run() permission(Admin) {}
}
`);
  const numericBoolean = compileKotodamaProgram(`
seiyaku MetaNumericBoolean {
  meta { vector: 1; }
  kotoage fn run() permission(Admin) {}
}
`);
  const stringBoolean = compileKotodamaProgram(`
seiyaku MetaStringBoolean {
  meta { vector: "x"; }
  kotoage fn run() permission(Admin) {}
}
`);
  const badFeatureItem = compileKotodamaProgram(`
seiyaku MetaBadFeatureItem {
  meta { features: [1]; }
  kotoage fn run() permission(Admin) {}
}
`);
  const unknownFeature = compileKotodamaProgram(`
seiyaku MetaUnknownFeature {
  meta { features: [vector, bad]; }
  kotoage fn run() permission(Admin) {}
}
`);
  const invalidAbi = compileKotodamaProgram(`
seiyaku MetaInvalidAbi {
  meta { abi: 2; }
  kotoage fn run() permission(Admin) {}
}
`);
  const negativeCycles = compileKotodamaProgram(`
seiyaku MetaNegativeCycles {
  meta { cycles: -1; }
  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(
    badParamType.diagnostics[0].message,
    "parser error: {error}: expected identifier but found Number(123)",
  );
  assert.equal(
    badMetaKey.diagnostics[0].message,
    "parser error: {error}: expected identifier but found Number(123)",
  );
  assert.equal(
    unknownMetaKey.diagnostics[0].message,
    "parser error: unknown meta numeric key 'unknown'",
  );
  assert.equal(
    numericBoolean.diagnostics[0].message,
    "parser error: unknown meta numeric key 'vector'",
  );
  assert.equal(
    stringBoolean.diagnostics[0].message,
    'parser error: {error}: expected number, boolean, or string list but found String("x")',
  );
  assert.equal(
    badFeatureItem.diagnostics[0].message,
    "parser error: {error}: expected string literal or identifier but found Number(1)",
  );
  assert.equal(
    unknownFeature.diagnostics[0].message,
    "parser error: unknown feature 'bad' in meta features list",
  );
  assert.equal(
    invalidAbi.diagnostics[0].message,
    "parser error: meta key 'abi' value 2 is not supported in the first release (expected 1)",
  );
  assert.equal(
    negativeCycles.diagnostics[0].message,
    "parser error: meta key 'cycles' value -1 must be non-negative",
  );
});

test("Kotodama compiler SDK emits kotoba localization metadata", () => {
  const compiled = compileKotodamaProgram(`
kotoba {
  "E0002": { ja: "後", en: "after" }
}

seiyaku KotobaDemo {
  kotoba {
    E0001: { ja: "準備", en: "ready" }
  }

  kotoage fn run() {
    info("ready");
  }
}
`);
  const duplicateId = compileKotodamaProgram(`
seiyaku DuplicateKotoba {
  kotoba {
    E0001: { en: "ready" },
    E0001: { en: "again" },
  }

  hajimari() {
    info("ready");
  }
}
`);
  const duplicateLang = compileKotodamaProgram(`
seiyaku DuplicateKotobaLang {
  kotoba {
    E0001: { en: "ready", en: "again" }
  }

  hajimari() {
    info("ready");
  }
}
`);

  const cntrBody = noritoPayloadBody(readArtifactSection(compiled.artifactBytes, 17).payload);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.manifest?.kotoba, [
    {
      msg_id: "E0001",
      translations: [
        { lang: "en", text: "ready" },
        { lang: "ja", text: "準備" },
      ],
    },
    {
      msg_id: "E0002",
      translations: [
        { lang: "en", text: "after" },
        { lang: "ja", text: "後" },
      ],
    },
  ]);
  assert.notEqual(cntrBody.indexOf(Buffer.from("E0001", "utf8")), -1);
  assert.notEqual(cntrBody.indexOf(Buffer.from("準備", "utf8")), -1);
  assert.equal(duplicateId.artifactBytes.length, 0);
  assert.equal(duplicateId.diagnostics.length, 1);
  assert.match(duplicateId.diagnostics[0].message, /duplicate kotoba key `E0001`/);
  assert.equal(duplicateLang.artifactBytes.length, 0);
  assert.equal(duplicateLang.diagnostics.length, 1);
  assert.match(duplicateLang.diagnostics[0].message, /repeats language `en`/);
});

test("Kotodama compiler SDK validates explicit contract meta feature requests", () => {
  const unusedVector = compileKotodamaProgram(`
seiyaku UnusedVectorFeature {
  meta {
    vector: true,
  }

  hajimari() {
    info("ready");
  }
}
`);
  const forbiddenVector = compileKotodamaProgram(`
seiyaku ForbiddenVectorFeature {
  meta {
    vector: false,
  }

  kotoage fn main() {
    setvl(8);
  }
}
`);

  assert.equal(unusedVector.artifactBytes.length, 0);
  assert.equal(unusedVector.diagnostics.length, 1);
  assert.match(unusedVector.diagnostics[0].message, /meta requests vector but no vector opcodes are emitted/);
  assert.equal(forbiddenVector.artifactBytes.length, 0);
  assert.equal(forbiddenVector.diagnostics.length, 1);
  assert.match(forbiddenVector.diagnostics[0].message, /meta disables vector but vector opcodes are emitted/);
});

test("Kotodama compiler SDK emits compact Norito interface metadata without embedded debug by default", () => {
  const compiled = compileKotodamaProgram(`
seiyaku CompactMetadata {
  hajimari() {
    info("ready");
  }
}
`);
  const cntr = readArtifactSection(compiled.artifactBytes, 17);
  const next = readArtifactSection(compiled.artifactBytes, cntr.end);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(cntr.tag, "CNTR");
  assert.equal(Buffer.from(cntr.payload.subarray(6, 22)).toString("hex"), "9c456132dfb6171e734d4d30527bddcc");
  assert.equal(cntr.payload[39], 0x02);
  assert.equal(cntr.payload[40], 0x1b);
  assert.equal(cntr.payload[41], 0x1a);
  assert.notEqual(next.tag, "DBG1");
});

test("Kotodama compiler SDK can opt into embedded DBG1 metadata", () => {
  const compiled = compileKotodamaProgram(`
seiyaku CompactMetadata {
  hajimari() {
    info("ready");
  }
}
`, { embedDebug: true });
  const cntr = readArtifactSection(compiled.artifactBytes, 17);
  const dbg = readArtifactSection(compiled.artifactBytes, cntr.end);

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(cntr.tag, "CNTR");
  assert.equal(dbg.tag, "DBG1");
  assert.equal(Buffer.from(dbg.payload.subarray(6, 22)).toString("hex"), "f3ef2532efe3b1a38edaa77584baa2ce");
  assert.equal(dbg.payload[39], 0x02);
});

test("Kotodama compiler SDK emits compact literal payloads and raw state paths", () => {
  const compiled = compileKotodamaProgram(`
seiyaku LiteralPayloads {
  state int balance;
  state AccountId custodian;
  state AssetDefinitionId asset_id;
  state DomainId domain_id;

  kotoage fn main() permission(Admin) {
    let account = account_id("ed01200102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20");
    let asset = asset_definition("61CtjvNd9T3THAR65GsMVHr82Bjc");
    let domain_ref = domain("wonder.universal");
    let key = name("cursor");
    let value = json!{ query: "sc_dummy", cursor: 1 };

    balance = balance + 1;
    set_account_detail(account, key, value);
    register_domain(domain_ref);
    mint_asset(account, asset, 1);
    custodian = account;
    asset_id = asset;
    domain_id = domain_ref;
  }
}
`);
  const entries = readLiteralTableEntries(compiled.artifactBytes);
  const payloadBodies = entries.map((entry) => ({
    type: entry.type,
    length: entry.length,
    body: entry.type === 6 || entry.type === 9 ? entry.payload : noritoPayloadBody(entry.payload),
  }));

  assert.deepEqual(compiled.diagnostics, []);
  assert.equal(payloadBodies.some((entry) => entry.type === 3 && entry.body.toString("utf8").includes("state:balance")), false);
  assert.ok(payloadBodies.some((entry) => entry.type === 3 && entry.body.equals(Buffer.from("0762616c616e6365", "hex"))));
  assert.ok(payloadBodies.some((entry) => entry.type === 3 && entry.body.equals(Buffer.from("06637572736f72", "hex"))));
  assert.ok(payloadBodies.some((entry) => (
    entry.type === 4
    && entry.body.subarray(2).toString("utf8") === "{\"cursor\":1,\"query\":\"sc_dummy\"}"
  )));
  assert.ok(payloadBodies.some((entry) => (
    entry.type === 8
    && entry.body.equals(Buffer.from("0706776f6e6465720a09756e6976657273616c", "hex"))
  )));
  assert.ok(payloadBodies.some((entry) => (
    entry.type === 1
    && entry.length === 119
    && entry.body.subarray(0, 14).equals(Buffer.from("000000004a210000000000000001", "hex"))
  )));
  assert.ok(payloadBodies.some((entry) => (
    entry.type === 2
    && entry.length === 72
    && entry.body.length === 32
    && entry.body.every((byte, index) => index % 2 === 0 ? byte === 1 : true)
  )));
});

test("Kotodama compiler SDK mirrors account_id alias literal resolution", () => {
  const canonicalPublicKey = "ed01200102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
  const alias = compileKotodamaProgram(`
seiyaku AccountAliasPointers {
  kotoage fn run() permission(Admin) {
    let account = account_id("merchant@paynet");
    let malformed = account_id("merchant@");
    let qualified = account_id("merchant@bank.paynet");
    set_account_detail(account, name("status"), json!{ ok: true });
    set_account_detail(malformed, name("status"), json!{ ok: false });
    set_account_detail(qualified, name("status"), json!{ ok: true });
  }
}
`);
  const canonical = compileKotodamaProgram(`
seiyaku CanonicalAccountPointer {
  kotoage fn run() permission(Admin) {
    let account = account_id("${canonicalPublicKey}");
    set_account_detail(account, name("status"), json!{ ok: true });
  }
}
`);
  const invalid = compileKotodamaProgram(`
seiyaku InvalidAccountPointer {
  kotoage fn run() permission(Admin) {
    let account = account_id("merchant");
  }
}
`);
  const aliasCode = readArtifactCode(alias.artifactBytes);
  const canonicalCode = readArtifactCode(canonical.artifactBytes);
  const aliasLiteralEntries = readLiteralTableEntries(alias.artifactBytes);

  assert.deepEqual(alias.diagnostics, []);
  assert.notEqual(aliasCode.indexOf(syscallNeedle(0xa7)), -1, "alias-shaped literals must resolve through RESOLVE_ACCOUNT_ALIAS");
  assert.ok(aliasLiteralEntries.some((entry) => entry.type === 6), "alias resolver should publish blob literal bytes");
  assert.deepEqual(canonical.diagnostics, []);
  assert.equal(canonicalCode.indexOf(syscallNeedle(0xa7)), -1, "canonical account literals must stay static");
  assert.equal(invalid.artifactBytes.length, 0);
  assert.equal(invalid.diagnostics.length, 1);
  assert.match(invalid.diagnostics[0].message, /invalid AccountId literal `merchant`/);
});

test("Kotodama compiler SDK emits AssetId pointer constructors and state metadata", () => {
  const assetDefinition = "61CtjvNd9T3THAR65GsMVHr82Bjc";
  const account = renderCanonicalAccountIdLiteralFromPublicKeyLiteral(
    "ed01200102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20",
  );
  assert.equal(typeof account, "string");

  const assetId = `${assetDefinition}#${account}`;
  const scopedAssetId = `${assetId}#dataspace:7`;
  const compiled = compileKotodamaProgram(`
seiyaku AssetIdPointers {
  state AssetId LastAsset;
  state Assets: Map<AssetId, AssetId>;

  kotoage fn run() permission(Admin) {
    let asset: AssetId = asset_id(${JSON.stringify(assetId)});
    let scoped: AssetId = asset_id!(${JSON.stringify(scopedAssetId)});
    let raw = norito_bytes("00");
    let from_bytes: AssetId = asset_id(raw);
    LastAsset = scoped;
    Assets[asset] = scoped;
    let stored = Assets[asset];
    LastAsset = stored;
    Assets[from_bytes] = scoped;
    info(1);
  }
}
`);
  const entries = readLiteralTableEntries(compiled.artifactBytes);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(entries.some((entry) => entry.type === 7), "missing AssetId literal pointer");
  assert.notEqual(code.indexOf(syscallNeedle(0x5e)), -1, "missing POINTER_FROM_NORITO syscall");
  assert.deepEqual(compiled.manifest?.states, [
    { name: "LastAsset", type_name: "AssetId" },
    { name: "Assets", type_name: "map<AssetId, AssetId>" },
  ]);
});

test("Kotodama compiler SDK rejects invalid AssetId constructor arguments semantically", () => {
  const compiled = compileKotodamaProgram(`
seiyaku InvalidAssetIdPointer {
  kotoage fn run() permission(Admin) {
    let asset = asset_id(1);
  }
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.equal(compiled.diagnostics.length, 1);
  assert.match(
    compiled.diagnostics[0].message,
    /asset_id expects string, matching pointer type, or Blob\|bytes \(NoritoBytes\)/,
  );
});

test("Kotodama compiler SDK rejects Json arguments for non-Json pointer constructors semantically", () => {
  const invalidAccount = compileKotodamaProgram(`
seiyaku InvalidJsonPointerConstructor {
  fn helper() {
    let account = account_id(json!{ owner: "alice" });
  }

  kotoage fn run() permission(Admin) {}
}
`);
  const validJson = compileKotodamaProgram(`
seiyaku ValidJsonPointerConstructor {
  fn helper() {
    let payload = json(json!{ owner: "alice" });
    info(1);
  }

  kotoage fn run() permission(Admin) {
    helper();
  }
}
`);

  assert.equal(invalidAccount.artifactBytes.length, 0);
  assert.equal(invalidAccount.diagnostics.length, 1);
  assert.match(
    invalidAccount.diagnostics[0].message,
    /account_id expects string, matching pointer type, or Blob\|bytes \(NoritoBytes\)/,
  );
  assert.deepEqual(validJson.diagnostics, []);
});

test("Kotodama compiler SDK emits NftId pointer state and map metadata", () => {
  const nftId = "n0$wonderland.universal";
  const compiled = compileKotodamaProgram(`
seiyaku NftIdPointers {
  state NftId LastNft;
  state Nfts: Map<NftId, NftId>;

  kotoage fn run() permission(Admin) {
    let nft: NftId = nft_id!(${JSON.stringify(nftId)});
    let raw = norito_bytes!("00");
    let from_bytes: NftId = nft_id(raw);
    LastNft = nft;
    Nfts[nft] = nft;
    let current = LastNft;
    let stored = Nfts[nft];
    LastNft = stored;
    Nfts[from_bytes] = current;
    info(1);
  }
}
`);
  const entries = readLiteralTableEntries(compiled.artifactBytes);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(entries.some((entry) => entry.type === 5), "missing NftId literal pointer");
  assert.notEqual(code.indexOf(syscallNeedle(0x5e)), -1, "missing POINTER_FROM_NORITO syscall");
  assert.deepEqual(compiled.manifest?.states, [
    { name: "LastNft", type_name: "NftId" },
    { name: "Nfts", type_name: "map<NftId, NftId>" },
  ]);
});

test("Kotodama compiler SDK lowers mixed direct and literal transfer operands", () => {
  const compiled = compileKotodamaProgram(`
seiyaku MixedLiteralTransfer {
  kotoage fn main() permission(MarketUser) {
    let caller = authority();
    let pool = account_id("ed01200102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20");
    let asset = asset_definition("61CtjvNd9T3THAR65GsMVHr82Bjc");
    transfer_asset(caller, pool, asset, 10);
  }
}
`, { sourceName: "mixed_literal_transfer.ko" });

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.sourceMap, [
    {
      function_name: "main",
      pc_start: 0,
      pc_end: 292,
      source_path: "mixed_literal_transfer.ko",
      line: 3,
      column: 14,
    },
  ]);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ([
      entry.function_name,
      entry.pc_start,
      entry.pc_end,
      entry.bytecode_bytes,
      entry.bytecode_words,
      entry.frame_bytes,
    ])),
    [["main", 0, 292, 292, 73, 64]],
  );
});

test("Kotodama compiler SDK derives exact asset access from static pointer locals", () => {
  const sourcePublicKey = "ed01200102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
  const destinationPublicKey = "ed01202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f40";
  const assetDefinition = "61CtjvNd9T3THAR65GsMVHr82Bjc";
  const sourceAccount = renderCanonicalAccountIdLiteralFromPublicKeyLiteral(sourcePublicKey);
  const destinationAccount = renderCanonicalAccountIdLiteralFromPublicKeyLiteral(destinationPublicKey);

  assert.notEqual(sourceAccount, null);
  assert.notEqual(destinationAccount, null);

  const sourceAsset = `asset:${assetDefinition}#${sourceAccount}`;
  const destinationAsset = `asset:${assetDefinition}#${destinationAccount}`;
  const compiled = compileKotodamaProgram(`
seiyaku StaticPointerLocalAccess {
  kotoage fn main() permission(Admin) {
    let asset = asset_definition("${assetDefinition}");
    let source = account_id("${sourcePublicKey}");
    let destination = account_id("${destinationPublicKey}");
    transfer_asset(source, destination, asset, 10);
    mint_asset(destination, asset, 5);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.read_keys, [
    sourceAsset,
    `account:${sourceAccount}`,
    `asset_def:${assetDefinition}`,
    destinationAsset,
    `account:${destinationAccount}`,
  ]);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.write_keys, [
    sourceAsset,
    destinationAsset,
    `asset_def:${assetDefinition}`,
  ]);
  assert.equal(compiled.manifest?.entrypoints[0]?.access_hints_complete, true);
  assert.equal(compiled.manifest?.entrypoints[0]?.read_keys.includes("*"), false);
  assert.equal(compiled.manifest?.entrypoints[0]?.write_keys.includes("*"), false);
});

test("Kotodama compiler SDK keeps static asset access hints for runtime-resolved accounts", () => {
  const sourcePublicKey = "ed01200102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
  const assetDefinition = "61CtjvNd9T3THAR65GsMVHr82Bjc";
  const sourceAccount = renderCanonicalAccountIdLiteralFromPublicKeyLiteral(sourcePublicKey);

  assert.notEqual(sourceAccount, null);

  const sourceAsset = `asset:${assetDefinition}#${sourceAccount}`;
  const compiled = compileKotodamaProgram(`
seiyaku RuntimeAccountStaticAssetAccess {
  kotoage fn transfer() permission(Admin) {
    transfer_asset(
      account_id("${sourcePublicKey}"),
      account_id("merchant@paynet"),
      asset_definition("${assetDefinition}"),
      1
    );
  }

  kotoage fn mint() permission(Admin) {
    mint_asset(account_id("merchant@paynet"), asset_definition("${assetDefinition}"), 1);
  }

  kotoage fn burn() permission(Admin) {
    burn_asset(account_id("merchant@"), asset_definition("${assetDefinition}"), 1);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  const transfer = compiled.manifest?.entrypoints.find((entry) => entry.name === "transfer");
  const mint = compiled.manifest?.entrypoints.find((entry) => entry.name === "mint");
  const burn = compiled.manifest?.entrypoints.find((entry) => entry.name === "burn");

  assert.deepEqual(transfer?.read_keys, [
    sourceAsset,
    `account:${sourceAccount}`,
    `asset_def:${assetDefinition}`,
  ]);
  assert.deepEqual(transfer?.write_keys, [
    sourceAsset,
    `asset_def:${assetDefinition}`,
  ]);
  assert.deepEqual(mint?.read_keys, [`asset_def:${assetDefinition}`]);
  assert.deepEqual(mint?.write_keys, [`asset_def:${assetDefinition}`]);
  assert.deepEqual(burn?.read_keys, [`asset_def:${assetDefinition}`]);
  assert.deepEqual(burn?.write_keys, [`asset_def:${assetDefinition}`]);
  for (const entrypoint of [transfer, mint, burn]) {
    assert.equal(entrypoint?.access_hints_complete, true);
    assert.deepEqual(entrypoint?.access_hints_skipped, []);
    assert.equal(entrypoint?.read_keys.some((key) => key === "*" || key.endsWith(":*")), false);
    assert.equal(entrypoint?.write_keys.some((key) => key === "*" || key.endsWith(":*")), false);
  }
});

test("Kotodama compiler SDK orders entrypoint access keys like Rust metadata", () => {
  const sourcePublicKey = "ed01200102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
  const destinationPublicKey = "ed01202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f40";
  const assetDefinition = "61CtjvNd9T3THAR65GsMVHr82Bjc";
  const sourceAccount = renderCanonicalAccountIdLiteralFromPublicKeyLiteral(sourcePublicKey);
  const destinationAccount = renderCanonicalAccountIdLiteralFromPublicKeyLiteral(destinationPublicKey);

  assert.notEqual(sourceAccount, null);
  assert.notEqual(destinationAccount, null);

  const sourceAsset = `asset:${assetDefinition}#${sourceAccount}`;
  const destinationAsset = `asset:${assetDefinition}#${destinationAccount}`;
  const compiled = compileKotodamaProgram(`
seiyaku OrderedAccess {
  state bool done;

  kotoage fn main() permission(Admin) {
    let asset = asset_definition("${assetDefinition}");
    let source = account_id("${sourcePublicKey}");
    let destination = account_id("${destinationPublicKey}");
    transfer_asset(source, destination, asset, 10);
    done = true;
    transfer_asset(authority(), destination, asset, 1);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.write_keys, [
    "state:done",
    sourceAsset,
    destinationAsset,
    `asset:${assetDefinition}:$authority`,
  ]);
});

test("Kotodama compiler SDK matches current Rust asset_ops authority access metadata", () => {
  const source = readFileSync(
    new URL("../../../crates/kotodama_lang/src/samples/asset_ops.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "asset_ops.ko" });
  const assetDefinition = "6pEP9RjNoZ7beWkT3pLfKoM1dyfi";

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.manifest?.entrypoints, [
    {
      name: "execute",
      kind: { kind: "Public", value: null },
      params: [],
      return_type: null,
      permission: "Admin",
      read_keys: [
        `asset:${assetDefinition}:$authority`,
        "account:$authority",
        `asset_def:${assetDefinition}`,
      ],
      write_keys: [
        `asset:${assetDefinition}:$authority`,
        `asset_def:${assetDefinition}`,
      ],
      access_hints_complete: true,
      access_hints_skipped: [],
      triggers: [],
    },
  ]);
  assert.deepEqual(compiled.manifest?.access_set_hints, {
    read_keys: [
      "account:$authority",
      `asset:${assetDefinition}:$authority`,
      `asset_def:${assetDefinition}`,
    ],
    write_keys: [
      `asset:${assetDefinition}:$authority`,
      `asset_def:${assetDefinition}`,
    ],
    dynamic_reads: [],
    dynamic_writes: [],
  });
  assert.deepEqual(compiled.manifest?.states, []);
});

test("Kotodama compiler SDK derives exact account-detail and domain host access", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ExactHostAccess {
  const domain_literal: String = "wonder.universal";

  kotoage fn mark() permission(Admin) {
    let key = name("cursor");
    set_account_detail(authority(), key, json("{}"));
  }

  kotoage fn pure() -> int {
    return poseidon2(1, 2);
  }

  kotoage fn move_domain() permission(Admin) {
    let domain_id = domain(domain_literal);
    transfer_domain(authority(), domain_id, authority());
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0], {
    name: "mark",
    kind: { kind: "Public", value: null },
    params: [],
    return_type: null,
    permission: "Admin",
    read_keys: [
      "account:$authority",
      "account.detail:$authority:cursor",
    ],
    write_keys: ["account.detail:$authority:cursor"],
    access_hints_complete: true,
    access_hints_skipped: [],
    triggers: [],
  });
  assert.deepEqual(compiled.manifest?.entrypoints[1], {
    name: "pure",
    kind: { kind: "Public", value: null },
    params: [],
    return_type: "int",
    permission: null,
    read_keys: [],
    write_keys: [],
    access_hints_complete: true,
    access_hints_skipped: [],
    triggers: [],
  });
  assert.deepEqual(compiled.manifest?.entrypoints[2]?.read_keys, [
    "domain:wonder.universal",
    "account:$authority",
  ]);
  assert.deepEqual(compiled.manifest?.entrypoints[2]?.write_keys, [
    "domain:wonder.universal",
  ]);
  assert.deepEqual(compiled.manifest?.access_set_hints, {
    read_keys: [
      "account.detail:$authority:cursor",
      "account:$authority",
      "domain:wonder.universal",
    ],
    write_keys: [
      "account.detail:$authority:cursor",
      "domain:wonder.universal",
    ],
    dynamic_reads: [],
    dynamic_writes: [],
  });
});

test("Kotodama compiler SDK emits native escrow syscalls and incomplete access reports", () => {
  const compiled = compileKotodamaProgram(`
seiyaku NativeEscrowBuiltins {
  kotoage fn run() permission(Admin) {
    let evidence = norito_bytes("00");
    escrow_open_offer(name("aitai_offer"), asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), 10, evidence);
    escrow_accept(name("aitai_offer"));
    escrow_mark_payment_sent(name("aitai_offer"));
    escrow_release(name("aitai_offer"));
    escrow_cancel(name("aitai_offer"));
    escrow_open_dispute(name("aitai_offer"), evidence);
    escrow_resolve_dispute(name("aitai_offer"), 6, 4, evidence);
  }
}
`);
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  for (const syscall of [0xb8, 0xb9, 0xba, 0xbb, 0xbc, 0xbd, 0xbe]) {
    assert.notEqual(code.indexOf(syscallNeedle(syscall)), -1, `missing syscall 0x${syscall.toString(16)}`);
  }
  assert.equal(compiled.manifest?.entrypoints[0]?.access_hints_complete, false);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.access_hints_skipped, [
    "opaque ISI access is not compiler-resolved",
  ]);
  assert.equal(compiled.manifest?.access_set_hints, null);
});

test("Kotodama compiler SDK keeps native escrow rows Rust-shaped", () => {
  const source = readFileSync(
    new URL("../../../crates/kotodama_lang/src/samples/native_escrow.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "native_escrow.ko" });

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ({
      function_name: entry.function_name,
      pc_start: entry.pc_start,
      pc_end: entry.pc_end,
      bytecode_bytes: entry.bytecode_bytes,
      bytecode_words: entry.bytecode_words,
      frame_bytes: entry.frame_bytes,
    })),
    [
      { function_name: "main", pc_start: 0, pc_end: 8, bytecode_bytes: 8, bytecode_words: 2, frame_bytes: 8 },
      { function_name: "__entrypoint_impl__open_offer", pc_start: 8, pc_end: 292, bytecode_bytes: 284, bytecode_words: 71, frame_bytes: 72 },
      { function_name: "open_offer", pc_start: 292, pc_end: 956, bytecode_bytes: 664, bytecode_words: 166, frame_bytes: 48 },
      { function_name: "__entrypoint_impl__accept", pc_start: 956, pc_end: 1008, bytecode_bytes: 52, bytecode_words: 13, frame_bytes: 24 },
      { function_name: "accept", pc_start: 1008, pc_end: 1392, bytecode_bytes: 384, bytecode_words: 96, frame_bytes: 32 },
      { function_name: "__entrypoint_impl__mark_payment_sent", pc_start: 1392, pc_end: 1444, bytecode_bytes: 52, bytecode_words: 13, frame_bytes: 24 },
      { function_name: "mark_payment_sent", pc_start: 1444, pc_end: 1828, bytecode_bytes: 384, bytecode_words: 96, frame_bytes: 32 },
      { function_name: "__entrypoint_impl__release", pc_start: 1828, pc_end: 1880, bytecode_bytes: 52, bytecode_words: 13, frame_bytes: 24 },
      { function_name: "release", pc_start: 1880, pc_end: 2264, bytecode_bytes: 384, bytecode_words: 96, frame_bytes: 32 },
      { function_name: "__entrypoint_impl__cancel", pc_start: 2264, pc_end: 2316, bytecode_bytes: 52, bytecode_words: 13, frame_bytes: 24 },
      { function_name: "cancel", pc_start: 2316, pc_end: 2700, bytecode_bytes: 384, bytecode_words: 96, frame_bytes: 32 },
      { function_name: "__entrypoint_impl__dispute", pc_start: 2700, pc_end: 2756, bytecode_bytes: 56, bytecode_words: 14, frame_bytes: 24 },
      { function_name: "dispute", pc_start: 2756, pc_end: 3140, bytecode_bytes: 384, bytecode_words: 96, frame_bytes: 32 },
      { function_name: "__entrypoint_impl__resolve_dispute", pc_start: 3140, pc_end: 3300, bytecode_bytes: 160, bytecode_words: 40, frame_bytes: 64 },
      { function_name: "resolve_dispute", pc_start: 3300, pc_end: 5420, bytecode_bytes: 2120, bytecode_words: 530, frame_bytes: 48 },
    ],
  );
});

test("Kotodama compiler SDK leaves aggregate-only entrypoint access unreported like Rust", () => {
  const compiled = compileKotodamaProgram(`
seiyaku AggregateOnlyAccess {
  kotoage fn compute() -> (int, int) {
    return (1, 2);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.read_keys, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.write_keys, []);
  assert.equal(compiled.manifest?.entrypoints[0]?.access_hints_complete, null);
});

test("Kotodama compiler SDK keeps immediate tuple-local returns Rust-shaped", () => {
  const source = readFileSync(
    new URL("../../../crates/kotodama_lang/src/samples/tuple_return_demo.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "tuple_return_demo.ko" });
  const code = readArtifactCode(compiled.artifactBytes);
  const pair = compiled.budgetReport.find((entry) => entry.function_name === "pair");

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(pair);
  assert.deepEqual(
    Array.from({ length: 10 }, (_, index) => code.readUInt32LE(pair.pc_start + 76 + index * 4)),
    [
      ivmWord(0x20, 6, 0, 0),
      ivmWord(0x20, 6, 6, 1),
      ivmWord(0x01, 5, 24, 6),
      ivmWord(0x20, 5, 0, 0),
      ivmWord(0x20, 5, 5, 1),
      ivmWord(0x01, 6, 23, 5),
      ivmWord(0x20, 6, 8, 0),
      ivmWord(0x20, 5, 7, 0),
      ivmWord(0x20, 10, 6, 0),
      ivmWord(0x20, 11, 5, 0),
    ],
  );
});

test("Kotodama compiler SDK reuses dead nested int locals in DEX quote lowering", () => {
  const compiled = compileKotodamaProgram(`
seiyaku DexContract {
  fn quote_sell(reserve_in: int, reserve_out: int, amount_in: int) -> int {
    let effective = (amount_in * 997) / 1000;
    let num = reserve_out * effective;
    let den = reserve_in + effective;
    return num / den;
  }

  kotoage fn swap(trader: AccountId,
                  pool: AccountId,
                  input_asset: AssetDefinitionId,
                  output_asset: AssetDefinitionId,
                  amount_in: int,
                  reserve_in: int,
                  reserve_out: int) -> int permission(Admin) {
    let out = quote_sell(reserve_in, reserve_out, amount_in);
    transfer_asset(trader, pool, input_asset, amount_in);
    transfer_asset(pool, trader, output_asset, out);
    return out;
  }
}
`, { sourceName: "dex_contract.ko" });
  const code = readArtifactCode(compiled.artifactBytes);
  const quote = compiled.budgetReport.find((entry) => entry.function_name === "quote_sell");

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(quote);
  assert.deepEqual(
    Array.from({ length: 5 }, (_, index) => code.readUInt32LE(quote.pc_start + 144 + index * 4)),
    [
      ivmWord(0x14, 9, 7, 8),
      ivmWord(0x10, 8, 23, 9),
      ivmWord(0x01, 23, 24, 9),
      ivmWord(0x14, 9, 8, 23),
      ivmWord(0x20, 10, 9, 0),
    ],
  );
});

test("Kotodama compiler SDK keeps inline product-over-sum quote lowering Rust-shaped", () => {
  const compiled = compileKotodamaProgram(`
seiyaku InlineQuote {
  kotoage fn swap(trader: AccountId,
                  pool: AccountId,
                  input_asset: AssetDefinitionId,
                  output_asset: AssetDefinitionId,
                  amount_in: int,
                  reserve_in: int,
                  reserve_out: int) -> int permission(Admin) {
    let effective = (amount_in * 997) / 1000;
    let out = (reserve_out * effective) / (reserve_in + effective);
    transfer_asset(trader, pool, input_asset, amount_in);
    transfer_asset(pool, trader, output_asset, out);
    return out;
  }
}
`, { sourceName: "inline_quote.ko" });
  const code = readArtifactCode(compiled.artifactBytes);
  const impl = compiled.budgetReport.find((entry) => entry.function_name === "__entrypoint_impl__swap");

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(impl);
  assert.deepEqual(
    Array.from({ length: 3 }, (_, index) => code.readUInt32LE(impl.pc_start + 252 + index * 4)),
    [
      ivmWord(0x10, 4, 5, 2),
      ivmWord(0x01, 5, 6, 2),
      ivmWord(0x14, 2, 4, 5),
    ],
  );
});

test("Kotodama compiler SDK treats plain indexed map assignment as write-only", () => {
  const compiled = compileKotodamaProgram(`
seiyaku MapSetAccess {
  state Values: Map<Name, int>;

  kotoage fn set_value(key: Name) permission(Admin) {
    Values[key] = 7;
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.read_keys, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.write_keys, ["state:Values"]);
  assert.deepEqual(compiled.manifest?.access_set_hints, {
    read_keys: ["state:Values"],
    write_keys: ["state:Values"],
    dynamic_reads: [],
    dynamic_writes: [],
  });
});

test("Kotodama compiler SDK treats get_or_default as read-only map access", () => {
  const compiled = compileKotodamaProgram(`
seiyaku GetOrDefaultAccess {
  state Values: Map<Name, int>;

  view fn read(key: Name) -> int {
    return get_or_default(Values, key, 7);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.read_keys, ["state:Values"]);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.write_keys, []);
  assert.deepEqual(compiled.manifest?.access_set_hints, {
    read_keys: ["state:Values"],
    write_keys: [],
    dynamic_reads: [],
    dynamic_writes: [],
  });
});

test("Kotodama compiler SDK keeps helper access at contract scope without transitive entrypoint keys", () => {
  const compiled = compileKotodamaProgram(`
seiyaku HelperAccess {
  state bool Flag;
  state Values: Map<Name, int>;

  fn guard() {
    assert(Flag, "flag");
  }

  fn ensure_value(key: Name) {
    assert(Values.contains(key), "missing");
  }

  kotoage fn run(key: Name) permission(Admin) {
    guard();
    ensure_value(key);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.read_keys, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.write_keys, []);
  assert.deepEqual(compiled.manifest?.access_set_hints, {
    read_keys: ["state:Flag", "state:Values"],
    write_keys: [],
    dynamic_reads: [],
    dynamic_writes: [],
  });
});

test("Kotodama compiler SDK keeps account-detail helper access out of caller entrypoints", () => {
  const compiled = compileKotodamaProgram(`
seiyaku AccountDetailHelperAccess {
  fn write_detail() {
    set_account_detail(authority(), name!("example"), json!{ hello: "world" });
  }

  kotoage fn run() permission(Admin) {
    write_detail();
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.read_keys, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.write_keys, []);
  assert.deepEqual(compiled.manifest?.access_set_hints, {
    read_keys: [
      "account.detail:$authority:example",
      "account:$authority",
    ],
    write_keys: ["account.detail:$authority:example"],
    dynamic_reads: [],
    dynamic_writes: [],
  });
});

test("Kotodama compiler SDK emits default raw main entrypoint metadata", () => {
  const compiled = compileKotodamaProgram(`
fn main(from: AccountId) {
  let mfc = mfc_asset();
  register_asset(mfc, "MFC", 0, 1);
  mint_asset(from, mfc, 1337);
}

fn mfc_asset() -> AssetDefinitionId {
  let mfc = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
  return mfc;
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.manifest?.entrypoints, [
    {
      name: "main",
      kind: { kind: "Public", value: null },
      params: [{ name: "from", type_name: "AccountId" }],
      return_type: null,
      permission: null,
      read_keys: ["asset_def:62Fk4FPcMuLvW5QjDGNF2a4jAmjM"],
      write_keys: ["asset_def:62Fk4FPcMuLvW5QjDGNF2a4jAmjM"],
      access_hints_complete: true,
      access_hints_skipped: [],
      triggers: [],
    },
  ]);
  assert.deepEqual(compiled.manifest?.access_set_hints, {
    read_keys: ["asset_def:62Fk4FPcMuLvW5QjDGNF2a4jAmjM"],
    write_keys: ["asset_def:62Fk4FPcMuLvW5QjDGNF2a4jAmjM"],
    dynamic_reads: [],
    dynamic_writes: [],
  });
});

test("Kotodama compiler SDK matches direct entrypoint fixed parameter homes", () => {
  const cases = [
    [
      "direct_kaizen_unused_param",
      'seiyaku T { kaizen(value: int) permission(Admin) { info("ok"); } }',
      [["kaizen", 124, 24]],
    ],
    [
      "direct_kaizen_used_param",
      "seiyaku T { kaizen(value: int) permission(Admin) { info(value); } }",
      [["kaizen", 40, 32]],
    ],
    [
      "direct_kaizen_empty",
      "seiyaku T { kaizen(value: int) permission(Admin) { } }",
      [["kaizen", 16, 24]],
    ],
    [
      "direct_hajimari_used_param",
      "seiyaku T { hajimari(value: int) { info(value); } }",
      [["hajimari", 40, 32]],
    ],
    [
      "direct_first_free_function",
      "fn start(value: int) { info(value); }",
      [["start", 40, 32]],
    ],
  ];

  for (const [name, source, rows] of cases) {
    const sourcePath = `/tmp/${name}.ko`;
    const compiled = compileKotodamaProgram(source, { sourceName: sourcePath });

    assert.deepEqual(compiled.diagnostics, []);
    assert.deepEqual(
      compiled.budgetReport.map((entry) => ({
        function_name: entry.function_name,
        bytecode_bytes: entry.bytecode_bytes,
        frame_bytes: entry.frame_bytes,
        source_path: entry.source_path,
      })),
      rows.map(([functionName, bytecodeBytes, frameBytes]) => ({
        function_name: functionName,
        bytecode_bytes: bytecodeBytes,
        frame_bytes: frameBytes,
        source_path: sourcePath,
      })),
    );
  }
});

test("Kotodama compiler SDK reuses dead direct entrypoint local results", () => {
  const cases = [
    [
      "dead_int_then_int",
      "seiyaku T { hajimari() { let x = 2; let a = 1; } }",
      [["hajimari", 24, 16]],
    ],
    [
      "dead_poseidon_then_int",
      "seiyaku T { hajimari() { let digest = poseidon2(1, 2); let a = 1; } }",
      [["hajimari", 36, 32]],
    ],
    [
      "dead_call_then_int",
      "fn helper() -> int { return 1; }\nseiyaku T { hajimari() { let value = helper(); let a = 1; } }",
      [
        ["hajimari", 120, 16],
        ["helper", 160, 16],
      ],
    ],
  ];

  for (const [name, source, rows] of cases) {
    const sourcePath = `/tmp/${name}.ko`;
    const compiled = compileKotodamaProgram(source, { sourceName: sourcePath });

    assert.deepEqual(compiled.diagnostics, []);
    assert.deepEqual(
      compiled.budgetReport.map((entry) => ({
        function_name: entry.function_name,
        bytecode_bytes: entry.bytecode_bytes,
        frame_bytes: entry.frame_bytes,
        source_path: entry.source_path,
      })),
      rows.map(([functionName, bytecodeBytes, frameBytes]) => ({
        function_name: functionName,
        bytecode_bytes: bytecodeBytes,
        frame_bytes: frameBytes,
        source_path: sourcePath,
      })),
    );
  }
});

test("Kotodama compiler SDK matches docs meta header budget rows", () => {
  const source = readFileSync(
    new URL("../../../crates/ivm/docs/examples/10_meta_header.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "10_meta_header.ko" });

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ([
      entry.function_name,
      entry.bytecode_bytes,
      entry.frame_bytes,
    ])),
    [["hajimari", 52, 32]],
  );
  assert.equal(compiled.manifest?.features_bitmap, 3);
});

test("Kotodama compiler SDK matches docs detail-transfer literal frame rows", () => {
  const source = readFileSync(
    new URL("../../../crates/ivm/docs/examples/11_detail_and_transfer.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "11_detail_and_transfer.ko" });

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ([
      entry.function_name,
      entry.bytecode_bytes,
      entry.frame_bytes,
    ])),
    [["set_cursor_and_transfer", 624, 48]],
  );
});

test("Kotodama compiler SDK matches Rust aggregate scalar state rows", () => {
  const cases = [
    [
      "one_field_struct_state",
      "seiyaku T { struct One { value: int } state One stored; kotoage fn f(a: int) { stored = One(a); } }",
      [
        ["f", 348, 32],
        ["__entrypoint_impl__f", 300, 40],
      ],
    ],
    [
      "two_field_struct_state",
      "seiyaku T { struct Pair { first: int, second: int } state Pair stored; kotoage fn f(a: int, b: int) { stored = Pair(a, b); } }",
      [
        ["f", 480, 40],
        ["__entrypoint_impl__f", 468, 64],
      ],
    ],
    [
      "two_field_literal_struct_state",
      "seiyaku T { struct Pair { first: int, second: int } state Pair stored; kotoage fn f() { stored = Pair(1, 2); } }",
      [["f", 312, 48]],
    ],
  ];

  for (const [name, source, rows] of cases) {
    const sourcePath = `/tmp/${name}.ko`;
    const compiled = compileKotodamaProgram(source, { sourceName: sourcePath });

    assert.deepEqual(compiled.diagnostics, []);
    assert.deepEqual(
      compiled.budgetReport.map((entry) => ({
        function_name: entry.function_name,
        bytecode_bytes: entry.bytecode_bytes,
        frame_bytes: entry.frame_bytes,
        source_path: entry.source_path,
      })),
      rows.map(([functionName, bytecodeBytes, frameBytes]) => ({
        function_name: functionName,
        bytecode_bytes: bytecodeBytes,
        frame_bytes: frameBytes,
        source_path: sourcePath,
      })),
    );
  }
});

test("Kotodama compiler SDK matches docs struct-state budget rows", () => {
  const source = readFileSync(
    new URL("../../../crates/ivm/docs/examples/09_struct_and_state.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "09_struct_and_state.ko" });

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ([
      entry.function_name,
      entry.bytecode_bytes,
      entry.frame_bytes,
    ])),
    [
      ["set_pair", 480, 40],
      ["__entrypoint_impl__set_pair", 576, 64],
    ],
  );
});

test("Kotodama compiler SDK matches Rust-style for-loop wrapper rows", () => {
  const cases = [
    [
      "sum_to",
      "seiyaku T { kotoage fn sum_to(n: int) -> int { let acc = 0; for let i = 0; i < n; i++ { acc = acc + i; } return acc; } }",
      [
        ["sum_to", 356, 32],
        ["__entrypoint_impl__sum_to", 772, 56],
      ],
    ],
    [
      "sum_to_step_assign",
      "seiyaku T { kotoage fn sum_to(n: int) -> int { let acc = 0; for let i = 0; i < n; i = i + 1 { acc = acc + i; } return acc; } }",
      [
        ["sum_to", 356, 32],
        ["__entrypoint_impl__sum_to", 772, 56],
      ],
    ],
    [
      "loop_with_extra_local",
      "seiyaku T { kotoage fn f(n: int) -> int { let acc = 0; let extra = 2; for let i = 0; i < n; i++ { acc = acc + i + extra; } return acc; } }",
      [
        ["f", 356, 32],
        ["__entrypoint_impl__f", 800, 64],
      ],
    ],
  ];

  for (const [name, source, rows] of cases) {
    const sourcePath = `/tmp/${name}.ko`;
    const compiled = compileKotodamaProgram(source, { sourceName: sourcePath });

    assert.deepEqual(compiled.diagnostics, []);
    assert.deepEqual(
      compiled.budgetReport.map((entry) => ({
        function_name: entry.function_name,
        bytecode_bytes: entry.bytecode_bytes,
        frame_bytes: entry.frame_bytes,
        source_path: entry.source_path,
      })),
      rows.map(([functionName, bytecodeBytes, frameBytes]) => ({
        function_name: functionName,
        bytecode_bytes: bytecodeBytes,
        frame_bytes: frameBytes,
        source_path: sourcePath,
      })),
    );
  }
});

test("Kotodama compiler SDK matches docs range-for budget rows", () => {
  const source = readFileSync(
    new URL("../../../crates/ivm/docs/examples/05_range_for.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "05_range_for.ko" });

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ([
      entry.function_name,
      entry.bytecode_bytes,
      entry.frame_bytes,
    ])),
    [
      ["sum_to", 356, 32],
      ["__entrypoint_impl__sum_to", 772, 56],
    ],
  );
});

test("Kotodama compiler SDK matches Rust local map frame rows", () => {
  const cases = [
    [
      "map_new_only_void",
      "seiyaku T { kotoage fn f() { let m = Map::new(); } }",
      [["f", 44, 16]],
    ],
    [
      "map_set_only",
      "seiyaku T { kotoage fn f() { let m = Map::new(); m[1] = 42; } }",
      [["f", 68, 32]],
    ],
    [
      "map_get_unset_return",
      "seiyaku T { kotoage fn f() -> int { let m = Map::new(); let v = m[1]; return v; } }",
      [["f", 76, 32]],
    ],
    [
      "map_set_get_return",
      "seiyaku T { kotoage fn f() -> int { let m = Map::new(); m[1] = 42; let v = m[1]; return v; } }",
      [["f", 100, 32]],
    ],
    [
      "map_set_get_direct_return",
      "seiyaku T { kotoage fn f() -> int { let m = Map::new(); m[1] = 42; return m[1]; } }",
      [["f", 100, 32]],
    ],
  ];

  for (const [name, source, rows] of cases) {
    const sourcePath = `/tmp/${name}.ko`;
    const compiled = compileKotodamaProgram(source, { sourceName: sourcePath });

    assert.deepEqual(compiled.diagnostics, []);
    assert.deepEqual(
      compiled.budgetReport.map((entry) => ({
        function_name: entry.function_name,
        bytecode_bytes: entry.bytecode_bytes,
        frame_bytes: entry.frame_bytes,
        source_path: entry.source_path,
      })),
      rows.map(([functionName, bytecodeBytes, frameBytes]) => ({
        function_name: functionName,
        bytecode_bytes: bytecodeBytes,
        frame_bytes: frameBytes,
        source_path: sourcePath,
      })),
    );
  }
});

test("Kotodama compiler SDK matches docs map-ops budget rows", () => {
  const source = readFileSync(
    new URL("../../../crates/ivm/docs/examples/06_map_ops.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "06_map_ops.ko" });

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ([
      entry.function_name,
      entry.bytecode_bytes,
      entry.frame_bytes,
    ])),
    [["map_example", 100, 32]],
  );
});

test("Kotodama compiler SDK matches Rust require lowering rows", () => {
  const cases = [
    [
      "direct_require_false",
      "seiyaku T { kotoage fn f() { require(false); } }",
      [["f", 28, 24]],
    ],
    [
      "direct_require_true",
      "seiyaku T { kotoage fn f() { require(true); } }",
      [["f", 32, 24]],
    ],
    [
      "direct_require_false_then_info",
      'seiyaku T { kotoage fn f() { require(false); info("x"); } }',
      [["f", 136, 24]],
    ],
    [
      "private_require_false",
      "fn f() { require(false); }\nfn main() { f(); }",
      [
        ["main", 112, 16],
        ["f", 160, 24],
      ],
    ],
  ];

  for (const [name, source, rows] of cases) {
    const sourcePath = `/tmp/${name}.ko`;
    const compiled = compileKotodamaProgram(source, { sourceName: sourcePath });

    assert.deepEqual(compiled.diagnostics, []);
    assert.deepEqual(
      compiled.budgetReport.map((entry) => ({
        function_name: entry.function_name,
        bytecode_bytes: entry.bytecode_bytes,
        frame_bytes: entry.frame_bytes,
        source_path: entry.source_path,
      })),
      rows.map(([functionName, bytecodeBytes, frameBytes]) => ({
        function_name: functionName,
        bytecode_bytes: bytecodeBytes,
        frame_bytes: frameBytes,
        source_path: sourcePath,
      })),
    );
  }
});

test("Kotodama compiler SDK matches docs contract-flow require rows", () => {
  const source = readFileSync(
    new URL("../../../crates/ivm/docs/examples/19_contract_flow_test.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "19_contract_flow_test.ko" });

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ([
      entry.function_name,
      entry.bytecode_bytes,
      entry.frame_bytes,
    ])),
    [
      ["hajimari", 156, 24],
      ["increment", 320, 32],
      ["remember_caller", 188, 24],
      ["reject_me", 368, 24],
    ],
  );
});

test("Kotodama compiler SDK matches raw main fixed parameter frame homes", () => {
  const cases = [
    ["raw_param_info", "fn main(value: int) { info(value); }", 40, 32],
    ["raw_unused_param_info", "fn main(value: int) { info(1); }", 48, 32],
    ["raw_unused_param_empty", "fn main(value: int) { }", 16, 24],
    ["raw_two_used_ints", "fn main(a: int, b: int) { info(a); info(b); }", 72, 48],
    ["raw_two_unused", "fn main(a: int, b: int) { info(1); }", 56, 40],
    ["raw_three_used_ints", "fn main(a: int, b: int, c: int) { info(a); info(b); info(c); }", 104, 64],
    ["raw_account_unused", "fn main(account: AccountId) { }", 16, 24],
    ["raw_account_info_int", "fn main(account: AccountId) { info(1); }", 48, 32],
    [
      "raw_account_detail",
      'fn main(account: AccountId) { set_account_detail(account, name!("probe"), json!("\\"ok\\"")); }',
      256,
      40,
    ],
    [
      "raw_two_accounts_transfer",
      'fn main(from: AccountId, to: AccountId) { let mfc = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"); transfer_asset(from, to, mfc, 1); }',
      208,
      64,
    ],
    ["raw_name_info_int", "fn main(key: Name) { info(1); }", 48, 32],
  ];

  for (const [name, source, bytecodeBytes, frameBytes] of cases) {
    const sourcePath = `/tmp/${name}.ko`;
    const compiled = compileKotodamaProgram(source, { sourceName: sourcePath });

    assert.deepEqual(compiled.diagnostics, []);
    assert.deepEqual(compiled.budgetReport, [
      {
        function_name: "main",
        pc_start: 0,
        pc_end: bytecodeBytes,
        bytecode_bytes: bytecodeBytes,
        bytecode_words: bytecodeBytes / 4,
        frame_bytes: frameBytes,
        jump_span_words: bytecodeBytes / 4,
        jump_range_risk: false,
        source_path: sourcePath,
        line: 1,
        column: 4,
      },
    ]);
  }
});

test("Kotodama compiler SDK matches raw parameterized void helper call discards", () => {
  const cases = [
    [
      "void_no_params",
      "fn main() { helper(); }\nfn helper() { info(1); }",
      [
        ["main", 112, 16],
        ["helper", 172, 24],
      ],
    ],
    [
      "void_one_int",
      "fn main(value: int) { helper(value); }\nfn helper(value: int) { info(value); }",
      [
        ["main", 124, 24],
        ["helper", 172, 32],
      ],
    ],
    [
      "void_two_int",
      "fn main(a: int, b: int) { helper(a, b); }\nfn helper(a: int, b: int) { info(a); info(b); }",
      [
        ["main", 136, 40],
        ["helper", 212, 48],
      ],
    ],
    [
      "void_one_account",
      "fn main(account: AccountId) { helper(account); }\nfn helper(account: AccountId) { info(1); }",
      [
        ["main", 124, 24],
        ["helper", 180, 32],
      ],
    ],
    [
      "void_three_mixed",
      "fn main(from: AccountId, to: AccountId) { helper(from, to, 369); }\nfn helper(from: AccountId, to: AccountId, amount: int) { info(amount); }",
      [
        ["main", 164, 48],
        ["helper", 188, 48],
      ],
    ],
  ];

  for (const [name, source, rows] of cases) {
    const sourcePath = `/tmp/${name}.ko`;
    const compiled = compileKotodamaProgram(source, { sourceName: sourcePath });

    assert.deepEqual(compiled.diagnostics, []);
    assert.deepEqual(
      compiled.budgetReport.map((entry) => ({
        function_name: entry.function_name,
        bytecode_bytes: entry.bytecode_bytes,
        frame_bytes: entry.frame_bytes,
        source_path: entry.source_path,
      })),
      rows.map(([functionName, bytecodeBytes, frameBytes]) => ({
        function_name: functionName,
        bytecode_bytes: bytecodeBytes,
        frame_bytes: frameBytes,
        source_path: sourcePath,
      })),
    );
  }
});

test("Kotodama compiler SDK matches private transfer helper frame homes", () => {
  const assetDefinition = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
  const cases = [
    [
      "helper_transfer_asset_param",
      "fn main(from: AccountId, to: AccountId, mfc: AssetDefinitionId, amount: int) { send(from, to, mfc, amount); }\n"
        + "fn send(from_account: AccountId, to_account: AccountId, mfc: AssetDefinitionId, amount: int) { transfer_asset(from_account, to_account, mfc, amount); }",
      [
        ["main", 160, 72],
        ["send", 280, 80],
      ],
    ],
    [
      "send_literal_local",
      `fn main(from: AccountId, to: AccountId) { send_mfc(from, to, 369); }\n`
        + `fn send_mfc(from_account: AccountId, to_account: AccountId, amount: int) {\n`
        + `  let mfc = asset_definition!("${assetDefinition}");\n`
        + `  transfer_asset(from_account, to_account, mfc, amount);\n`
        + `}`,
      [
        ["main", 164, 48],
        ["send_mfc", 364, 72],
      ],
    ],
    [
      "send_helper_let_return",
      `fn main(from: AccountId, to: AccountId) { send_mfc(from, to, 369); }\n`
        + `fn send_mfc(from_account: AccountId, to_account: AccountId, amount: int) {\n`
        + `  let mfc = mfc_asset();\n`
        + `  transfer_asset(from_account, to_account, mfc, amount);\n`
        + `}\n`
        + `fn mfc_asset() -> AssetDefinitionId { let mfc = asset_definition!("${assetDefinition}"); return mfc; }`,
      [
        ["main", 164, 48],
        ["send_mfc", 364, 72],
        ["mfc_asset", 348, 16],
      ],
    ],
  ];

  for (const [name, source, rows] of cases) {
    const sourcePath = `/tmp/${name}.ko`;
    const compiled = compileKotodamaProgram(source, { sourceName: sourcePath });

    assert.deepEqual(compiled.diagnostics, []);
    assert.deepEqual(
      compiled.budgetReport.map((entry) => ({
        function_name: entry.function_name,
        bytecode_bytes: entry.bytecode_bytes,
        frame_bytes: entry.frame_bytes,
        source_path: entry.source_path,
      })),
      rows.map(([functionName, bytecodeBytes, frameBytes]) => ({
        function_name: functionName,
        bytecode_bytes: bytecodeBytes,
        frame_bytes: frameBytes,
        source_path: sourcePath,
      })),
    );
  }
});

test("Kotodama compiler SDK matches Rust-style for-loop transfer frame homes", () => {
  const assetDefinition = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
  const cases = [
    [
      "loop_info",
      "fn main() { loop_qty(); }\n"
        + "fn loop_qty() { for let qty = 369; qty <= 1337; qty++ { info(qty); } }",
      [
        ["main", 112, 16],
        ["loop_qty", 796, 32],
      ],
    ],
    [
      "loop_transfer_param",
      "fn main(from: AccountId, to: AccountId, mfc: AssetDefinitionId) { loop_mfc(from, to, mfc); }\n"
        + "fn loop_mfc(from_account: AccountId, to_account: AccountId, mfc: AssetDefinitionId) {\n"
        + "  for let qty = 369; qty <= 1337; qty++ {\n"
        + "    transfer_asset(from_account, to_account, mfc, qty);\n"
        + "  }\n"
        + "}",
      [
        ["main", 148, 56],
        ["loop_mfc", 916, 80],
      ],
    ],
    [
      "loop_transfer_helper_return",
      `fn main(from: AccountId, to: AccountId) { loop_mfc(from, to); }\n`
        + `fn loop_mfc(from_account: AccountId, to_account: AccountId) {\n`
        + `  let mfc = mfc_asset();\n`
        + `  for let qty = 369; qty <= 1337; qty++ {\n`
        + `    transfer_asset(from_account, to_account, mfc, qty);\n`
        + `  }\n`
        + `}\n`
        + `fn mfc_asset() -> AssetDefinitionId { let mfc = asset_definition!("${assetDefinition}"); return mfc; }`,
      [
        ["main", 136, 40],
        ["loop_mfc", 1000, 72],
        ["mfc_asset", 348, 16],
      ],
    ],
  ];

  for (const [name, source, rows] of cases) {
    const sourcePath = `/tmp/${name}.ko`;
    const compiled = compileKotodamaProgram(source, { sourceName: sourcePath });

    assert.deepEqual(compiled.diagnostics, []);
    assert.deepEqual(
      compiled.budgetReport.map((entry) => ({
        function_name: entry.function_name,
        bytecode_bytes: entry.bytecode_bytes,
        frame_bytes: entry.frame_bytes,
        source_path: entry.source_path,
      })),
      rows.map(([functionName, bytecodeBytes, frameBytes]) => ({
        function_name: functionName,
        bytecode_bytes: bytecodeBytes,
        frame_bytes: frameBytes,
        source_path: sourcePath,
      })),
    );
  }
});

test("Kotodama compiler SDK rematerializes static asset helper returns in raw register and mint paths", () => {
  const assetDefinition = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
  const helper = `fn mfc_asset() -> AssetDefinitionId { let mfc = asset_definition!("${assetDefinition}"); return mfc; }`;
  const cases = [
    [
      "register_helper",
      `fn main() { let mfc = mfc_asset(); register_asset(mfc, "MFC", 0, 1); }\n${helper}`,
      [
        ["main", 352, 40],
        ["mfc_asset", 244, 16],
      ],
    ],
    [
      "mint_helper",
      `fn main(from: AccountId) { let mfc = mfc_asset(); mint_asset(from, mfc, 1337); }\n${helper}`,
      [
        ["main", 336, 48],
        ["mfc_asset", 244, 16],
      ],
    ],
    [
      "register_mint_helper",
      `fn main(from: AccountId) { let mfc = mfc_asset(); register_asset(mfc, "MFC", 0, 1); let mfc = mfc_asset(); mint_asset(from, mfc, 1337); }\n${helper}`,
      [
        ["main", 680, 56],
        ["mfc_asset", 244, 16],
      ],
    ],
  ];

  for (const [name, source, rows] of cases) {
    const sourcePath = `/tmp/${name}.ko`;
    const compiled = compileKotodamaProgram(source, { sourceName: sourcePath });

    assert.deepEqual(compiled.diagnostics, []);
    assert.deepEqual(
      compiled.budgetReport.map((entry) => ({
        function_name: entry.function_name,
        bytecode_bytes: entry.bytecode_bytes,
        frame_bytes: entry.frame_bytes,
        source_path: entry.source_path,
      })),
      rows.map(([functionName, bytecodeBytes, frameBytes]) => ({
        function_name: functionName,
        bytecode_bytes: bytecodeBytes,
        frame_bytes: frameBytes,
        source_path: sourcePath,
      })),
    );
  }
});

test("Kotodama compiler SDK matches full MFC raw-main budget rows", () => {
  const source = readFileSync(
    new URL("../../../crates/ivm/tests/data/mfc.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "mfc.ko" });

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ([
      entry.function_name,
      entry.bytecode_bytes,
      entry.frame_bytes,
    ])),
    [
      ["main", 940, 72],
      ["send_mfc", 364, 72],
      ["loop_mfc", 1000, 72],
      ["mfc_asset", 452, 16],
    ],
  );
});

test("Kotodama compiler SDK matches AssetDefinitionId helper return frame shape", () => {
  const assetDefinition = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
  const cases = [
    [
      "helper_asset_direct_return",
      `fn main() { info(1); }\nfn mfc_asset() -> AssetDefinitionId { return asset_definition!("${assetDefinition}"); }`,
    ],
    [
      "helper_asset_let_return",
      `fn main() { info(1); }\nfn mfc_asset() -> AssetDefinitionId { let mfc = asset_definition!("${assetDefinition}"); return mfc; }`,
    ],
  ];

  for (const [name, source] of cases) {
    const sourcePath = `/tmp/${name}.ko`;
    const compiled = compileKotodamaProgram(source, { sourceName: sourcePath });

    assert.deepEqual(compiled.diagnostics, []);
    assert.deepEqual(compiled.budgetReport, [
      {
        function_name: "main",
        pc_start: 0,
        pc_end: 40,
        bytecode_bytes: 40,
        bytecode_words: 10,
        frame_bytes: 24,
        jump_span_words: 10,
        jump_range_risk: false,
        source_path: sourcePath,
        line: 1,
        column: 4,
      },
      {
        function_name: "mfc_asset",
        pc_start: 40,
        pc_end: 284,
        bytecode_bytes: 244,
        bytecode_words: 61,
        frame_bytes: 16,
        jump_span_words: 61,
        jump_range_risk: false,
        source_path: sourcePath,
        line: 2,
        column: 4,
      },
    ]);
  }
});

test("Kotodama compiler SDK emits contract dynamic state hints for map-parameter helpers", () => {
  const compiled = compileKotodamaProgram(`
seiyaku DynamicMapHelperAccess {
  state Values: Map<Name, int>;

  fn value_or_default(state Map<Name, int> values, key: Name) -> int {
    return values.ensure(key, 0);
  }

  kotoage fn run(key: Name) permission(Admin) {
    let value = value_or_default(Values, key);
    info(value);
  }
}
`);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.read_keys, []);
  assert.deepEqual(compiled.manifest?.entrypoints[0]?.write_keys, []);
  assert.equal(compiled.manifest?.access_set_hints, null);
  assert.deepEqual(compiled.manifest?.states, [
    { name: "Values", type_name: "map<Name, int>" },
  ]);
});

test("Kotodama compiler SDK rejects manual access attributes like upstream", () => {
  const compiled = compileKotodamaProgram(`
seiyaku ManualAccessAttr {
  #[access(read="*", write="*")]
  kotoage fn run() permission(Admin) {}
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.deepEqual(compiled.diagnostics, [
    {
      severity: "error",
      message: 'parser error: expected manual `#[access(...)]` hints are not supported in first-release Kotodama; access metadata is generated by the compiler but found Ident("access")',
      line: 3,
      column: 5,
    },
  ]);
});

test("Kotodama compiler SDK rejects unsupported attributes like upstream", () => {
  const compiled = compileKotodamaProgram(`
seiyaku UnknownAttr {
  #[memo]
  fn helper() {}
}
`);

  assert.equal(compiled.artifactBytes.length, 0);
  assert.deepEqual(compiled.diagnostics, [
    {
      severity: "error",
      message: 'parser error: expected expected attribute `test` or `テスト` but found Ident("memo")',
      line: 3,
      column: 5,
    },
  ]);
});

test("Kotodama compiler SDK mirrors Rust function-attribute parser diagnostics", () => {
  const attrBeforeState = compileKotodamaProgram(`
#[test]
state int Counter;
fn run() {}
`);
  const attrBeforeStruct = compileKotodamaProgram(`
#[test]
struct User { value: int }
fn run() {}
`);
  const invalidAttributeName = compileKotodamaProgram(`
#[123]
fn run() {}
`);
  const invalidFixtureValue = compileKotodamaProgram(`
#[test(fixture=true)]
fn run() {}
`, { mode: "test" });
  const invalidFixtureKey = compileKotodamaProgram(`
#[test(fn="seeded")]
fn run() {}
`, { mode: "test" });

  assert.equal(attrBeforeState.artifactBytes.length, 0);
  assert.equal(attrBeforeState.diagnostics[0].message, "parser error: expected function attributes must precede a function but found State");
  assert.equal(attrBeforeStruct.artifactBytes.length, 0);
  assert.equal(attrBeforeStruct.diagnostics[0].message, "parser error: {error}: expected function attributes must precede a function but found Struct");
  assert.equal(invalidAttributeName.artifactBytes.length, 0);
  assert.equal(invalidAttributeName.diagnostics[0].message, "parser error: expected expected attribute identifier but found Number(123)");
  assert.equal(invalidFixtureValue.artifactBytes.length, 0);
  assert.equal(invalidFixtureValue.diagnostics[0].message, "parser error: expected identifier or string literal but found True");
  assert.equal(invalidFixtureKey.artifactBytes.length, 0);
  assert.equal(invalidFixtureKey.diagnostics[0].message, "parser error: expected identifier but found Fn");
});

test("Kotodama compiler SDK keeps threshold escrow rows Rust-shaped", () => {
  const source = readFileSync(
    new URL("../../../crates/kotodama_lang/src/samples/threshold_escrow.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "threshold_escrow.ko" });

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ({
      function_name: entry.function_name,
      pc_start: entry.pc_start,
      pc_end: entry.pc_end,
      bytecode_bytes: entry.bytecode_bytes,
      bytecode_words: entry.bytecode_words,
      frame_bytes: entry.frame_bytes,
    })),
    [
      {
        function_name: "main",
        pc_start: 0,
        pc_end: 8,
        bytecode_bytes: 8,
        bytecode_words: 2,
        frame_bytes: 8,
      },
      {
        function_name: "assert_unopened",
        pc_start: 8,
        pc_end: 784,
        bytecode_bytes: 776,
        bytecode_words: 194,
        frame_bytes: 32,
      },
      {
        function_name: "assert_open",
        pc_start: 784,
        pc_end: 1556,
        bytecode_bytes: 772,
        bytecode_words: 193,
        frame_bytes: 32,
      },
      {
        function_name: "assert_payer",
        pc_start: 1556,
        pc_end: 1876,
        bytecode_bytes: 320,
        bytecode_words: 80,
        frame_bytes: 32,
      },
      {
        function_name: "__entrypoint_impl__open_escrow",
        pc_start: 1876,
        pc_end: 4016,
        bytecode_bytes: 2140,
        bytecode_words: 535,
        frame_bytes: 40,
      },
      {
        function_name: "open_escrow",
        pc_start: 4016,
        pc_end: 4400,
        bytecode_bytes: 384,
        bytecode_words: 96,
        frame_bytes: 32,
      },
      {
        function_name: "__entrypoint_impl__deposit",
        pc_start: 4400,
        pc_end: 5608,
        bytecode_bytes: 1208,
        bytecode_words: 302,
        frame_bytes: 64,
      },
      {
        function_name: "deposit",
        pc_start: 5608,
        pc_end: 5992,
        bytecode_bytes: 384,
        bytecode_words: 96,
        frame_bytes: 32,
      },
      {
        function_name: "release_if_ready",
        pc_start: 5992,
        pc_end: 7168,
        bytecode_bytes: 1176,
        bytecode_words: 294,
        frame_bytes: 48,
      },
      {
        function_name: "refund",
        pc_start: 7168,
        pc_end: 9488,
        bytecode_bytes: 2320,
        bytecode_words: 580,
        frame_bytes: 48,
      },
    ],
  );
  assert.ok(compiled.manifest?.access_set_hints?.read_keys.includes(
    "asset:62Fk4FPcMuLvW5QjDGNF2a4jAmjM:$authority",
  ));
  assert.ok(compiled.manifest?.access_set_hints?.read_keys.includes(
    "asset:62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76",
  ));

  const code = readArtifactCode(compiled.artifactBytes);
  const assertUnopened = compiled.budgetReport.find((entry) => entry.function_name === "assert_unopened");
  const assertPayer = compiled.budgetReport.find((entry) => entry.function_name === "assert_payer");
  const depositImpl = compiled.budgetReport.find((entry) => entry.function_name === "__entrypoint_impl__deposit");
  const releaseIfReady = compiled.budgetReport.find((entry) => entry.function_name === "release_if_ready");
  const refund = compiled.budgetReport.find((entry) => entry.function_name === "refund");
  assert.ok(assertUnopened);
  assert.deepEqual(
    Array.from({ length: 10 }, (_, index) => code.readUInt32LE(assertUnopened.pc_start + 116 + index * 4)),
    [
      ivmWord(0x60, 0, 0, 0xe0),
      ivmWord(0x60, 0, 0, 0x50),
      ivmWord(0x20, 23, 10, 0),
      ivmWord(0x20, 10, 23, 0),
      ivmWord(0x60, 0, 0, 0xe0),
      ivmWord(0x60, 0, 0, 0x53),
      ivmWord(0x20, 24, 10, 0),
      ivmWord(0x20, 23, 0, 0),
      ivmWord(0x0f, 9, 24, 23),
      ivmWord(0x23, 23, 9, 1),
    ],
  );
  assert.ok(assertPayer);
  assert.deepEqual(
    Array.from({ length: 9 }, (_, index) => code.readUInt32LE(assertPayer.pc_start + 132 + index * 4)),
    [
      ivmWord(0x20, 9, 10, 0),
      ivmWord(0x20, 10, 9, 0),
      ivmWord(0x60, 0, 0, 0xe0),
      ivmWord(0x20, 11, 0, 0),
      ivmWord(0x20, 11, 11, 1),
      ivmWord(0x60, 0, 0, 0x5e),
      ivmWord(0x20, 23, 10, 0),
      ivmWord(0x20, 10, 24, 0),
      ivmWord(0x60, 0, 0, 0xe0),
    ],
  );
  assert.ok(depositImpl);
  assert.deepEqual(
    Array.from({ length: 6 }, (_, index) => code.readUInt32LE(depositImpl.pc_start + 756 + index * 4)),
    [
      ivmWord(0x60, 0, 0, 0xa4),
      ivmWord(0x20, 8, 10, 0),
      ivmWord(0x20, 10, 24, 0),
      ivmWord(0x60, 0, 0, 0x69),
      ivmWord(0x20, 6, 10, 0),
      ivmWord(0x20, 10, 8, 0),
    ],
  );
  assert.ok(releaseIfReady);
  assert.equal(code.readUInt32LE(releaseIfReady.pc_start + 864), ivmWord(0x20, 8, 10, 0));
  assert.equal(code.readUInt32LE(releaseIfReady.pc_start + 964), ivmWord(0x20, 11, 8, 0));
  assert.ok(refund);
  assert.deepEqual(
    Array.from({ length: 5 }, (_, index) => code.readUInt32LE(refund.pc_start + 584 + index * 4)),
    [
      ivmWord(0x60, 0, 0, 0xa4),
      ivmWord(0x20, 23, 10, 0),
      ivmWord(0x20, 10, 24, 0),
      ivmWord(0x60, 0, 0, 0x69),
      ivmWord(0x20, 7, 10, 0),
    ],
  );
  assert.equal(code.readUInt32LE(refund.pc_start + 700), ivmWord(0x20, 11, 23, 0));
});

test("Kotodama compiler SDK keeps stablecoin helper and wrapper rows Rust-shaped", () => {
  const source = readFileSync(
    new URL("../../../crates/kotodama_lang/src/samples/stablecoin_simple.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "stablecoin_simple.ko" });

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ({
      function_name: entry.function_name,
      pc_start: entry.pc_start,
      pc_end: entry.pc_end,
      bytecode_bytes: entry.bytecode_bytes,
      bytecode_words: entry.bytecode_words,
      frame_bytes: entry.frame_bytes,
    })),
    [
      {
        function_name: "mintable_amount",
        pc_start: 0,
        pc_end: 620,
        bytecode_bytes: 620,
        bytecode_words: 155,
        frame_bytes: 48,
      },
      {
        function_name: "__entrypoint_impl__mintable_amount",
        pc_start: 620,
        pc_end: 820,
        bytecode_bytes: 200,
        bytecode_words: 50,
        frame_bytes: 64,
      },
      {
        function_name: "__entrypoint_impl__mint_stable",
        pc_start: 820,
        pc_end: 1748,
        bytecode_bytes: 928,
        bytecode_words: 232,
        frame_bytes: 144,
      },
      {
        function_name: "mint_stable",
        pc_start: 1748,
        pc_end: 3424,
        bytecode_bytes: 1676,
        bytecode_words: 419,
        frame_bytes: 88,
      },
    ],
  );
});

test("Kotodama compiler SDK keeps lending and perp helper rows Rust-shaped", () => {
  const fixtures = [
    {
      path: "../../../crates/kotodama_lang/src/samples/lending_simple.ko",
      sourceName: "lending_simple.ko",
      rows: [
        ["borrow", 0, 1140, 1140, 285, 80],
        ["collateral_ratio_bps", 1140, 1824, 684, 171, 56],
        ["__entrypoint_impl__borrow", 1824, 2612, 788, 197, 120],
        ["__entrypoint_impl__repay", 2612, 2856, 244, 61, 80],
        ["repay", 2856, 4076, 1220, 305, 56],
      ],
    },
    {
      path: "../../../crates/kotodama_lang/src/samples/perp_funding.ko",
      sourceName: "perp_funding.ko",
      rows: [
        ["settle_funding", 0, 1140, 1140, 285, 80],
        ["funding_payment", 1140, 1364, 224, 56, 80],
        ["__entrypoint_impl__settle_funding", 1364, 2228, 864, 216, 128],
        ["__entrypoint_impl__is_liquidatable", 2228, 2904, 676, 169, 72],
        ["is_liquidatable", 2904, 3992, 1088, 272, 48],
      ],
    },
  ];

  for (const fixture of fixtures) {
    const source = readFileSync(new URL(fixture.path, import.meta.url), "utf8");
    const compiled = compileKotodamaProgram(source, { sourceName: fixture.sourceName });

    assert.deepEqual(compiled.diagnostics, []);
    assert.deepEqual(
      compiled.budgetReport.map((entry) => ([
        entry.function_name,
        entry.pc_start,
        entry.pc_end,
        entry.bytecode_bytes,
        entry.bytecode_words,
        entry.frame_bytes,
      ])),
      fixture.rows,
    );
  }
});

test("Kotodama compiler SDK reuses true-arm discard registers in compare literal false arms", () => {
  const fixtures = [
    {
      path: "../../../crates/kotodama_lang/src/samples/lending_simple.ko",
      sourceName: "lending_simple.ko",
      functionName: "__entrypoint_impl__borrow",
      relativePc: 548,
      words: [
        ivmWord(0x20, 7, 0, 0),
        ivmWord(0x20, 5, 0, 0),
        ivmWord(0x20, 6, 5, 0),
      ],
    },
    {
      path: "../../../crates/kotodama_lang/src/samples/stablecoin_simple.ko",
      sourceName: "stablecoin_simple.ko",
      functionName: "__entrypoint_impl__mint_stable",
      relativePc: 676,
      words: [
        ivmWord(0x20, 6, 0, 0),
        ivmWord(0x20, 7, 0, 0),
        ivmWord(0x20, 5, 7, 0),
      ],
    },
  ];

  for (const fixture of fixtures) {
    const source = readFileSync(new URL(fixture.path, import.meta.url), "utf8");
    const compiled = compileKotodamaProgram(source, { sourceName: fixture.sourceName });
    const code = readArtifactCode(compiled.artifactBytes);
    const row = compiled.budgetReport.find((entry) => entry.function_name === fixture.functionName);

    assert.deepEqual(compiled.diagnostics, []);
    assert.ok(row);
    assert.deepEqual(
      Array.from({ length: fixture.words.length }, (_, index) => (
        code.readUInt32LE(row.pc_start + fixture.relativePc + index * 4)
      )),
      fixture.words,
    );
  }
});

test("Kotodama compiler SDK keeps PerpFunding helper register reuse Rust-shaped", () => {
  const source = readFileSync(
    new URL("../../../crates/kotodama_lang/src/samples/perp_funding.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "perp_funding.ko" });
  const code = readArtifactCode(compiled.artifactBytes);
  const fundingPayment = compiled.budgetReport.find((entry) => entry.function_name === "funding_payment");
  const isLiquidatable = compiled.budgetReport.find((entry) => entry.function_name === "__entrypoint_impl__is_liquidatable");

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(fundingPayment);
  assert.deepEqual(
    Array.from({ length: 5 }, (_, index) => code.readUInt32LE(fundingPayment.pc_start + 80 + index * 4)),
    [
      ivmWord(0x20, 9, 0, 0),
      ivmWord(0x20, 9, 9, 4),
      ivmWord(0x06, 9, 9, 26),
      ivmWord(0x20, 9, 9, 127),
      ivmWord(0x20, 9, 9, 127),
    ],
  );
  assert.deepEqual(
    Array.from({ length: 3 }, (_, index) => code.readUInt32LE(fundingPayment.pc_start + 148 + index * 4)),
    [
      ivmWord(0x20, 9, 9, 30),
      ivmWord(0x14, 8, 7, 9),
      ivmWord(0x20, 10, 8, 0),
    ],
  );
  assert.ok(isLiquidatable);
  assert.deepEqual(
    Array.from({ length: 3 }, (_, index) => code.readUInt32LE(isLiquidatable.pc_start + 600 + index * 4)),
    [
      ivmWord(0x08, 8, 12, 13),
      ivmWord(0x21, 8, 8, 1),
      ivmWord(0x20, 10, 8, 0),
    ],
  );
});

test("Kotodama compiler SDK keeps IrohaSwap rows Rust-shaped", () => {
  const source = readFileSync(
    new URL("../../../crates/kotodama_lang/src/samples/irohaswap.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "irohaswap.ko" });
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ([
      entry.function_name,
      entry.pc_start,
      entry.pc_end,
      entry.bytecode_bytes,
      entry.bytecode_words,
      entry.frame_bytes,
    ])),
    [
      ["init_pool", 0, 876, 876, 219, 64],
      ["min", 876, 1412, 536, 134, 48],
      ["reserve_of", 1412, 2212, 800, 200, 64],
      ["ensure_pool", 2212, 2544, 332, 83, 40],
      ["__entrypoint_impl__init_pool", 2544, 4600, 2056, 514, 112],
      ["__entrypoint_impl__deposit_liquidity", 4600, 8132, 3532, 883, 168],
      ["deposit_liquidity", 8132, 8936, 804, 201, 56],
      ["__entrypoint_impl__withdraw_liquidity", 8936, 12060, 3124, 781, 144],
      ["withdraw_liquidity", 12060, 12724, 664, 166, 48],
      ["__entrypoint_impl__swap", 12724, 17204, 4480, 1120, 160],
      ["swap", 17204, 19196, 1992, 498, 64],
    ],
  );
  const reserveOf = compiled.budgetReport.find((entry) => entry.function_name === "reserve_of");
  const ensurePool = compiled.budgetReport.find((entry) => entry.function_name === "ensure_pool");
  const initPoolImpl = compiled.budgetReport.find((entry) => entry.function_name === "__entrypoint_impl__init_pool");
  const depositLiquidityImpl = compiled.budgetReport.find((entry) => (
    entry.function_name === "__entrypoint_impl__deposit_liquidity"
  ));
  const withdrawLiquidityImpl = compiled.budgetReport.find((entry) => (
    entry.function_name === "__entrypoint_impl__withdraw_liquidity"
  ));
  const swapImpl = compiled.budgetReport.find((entry) => entry.function_name === "__entrypoint_impl__swap");

  assert.ok(reserveOf);
  assert.deepEqual(
    [14, 24, 28, 29, 30, 31, 103, 104].map((rel) => code.readUInt32LE(reserveOf.pc_start + rel * 4)),
    [
      ivmWord(0x20, 9, 10, 0),
      ivmWord(0x20, 8, 10, 0),
      ivmWord(0x20, 9, 10, 0),
      ivmWord(0x20, 8, 0, 0),
      ivmWord(0x0f, 7, 9, 8),
      ivmWord(0x41, 7, 0, 26),
      ivmWord(0x20, 8, 10, 0),
      ivmWord(0x20, 7, 8, 0),
    ],
  );
  assert.ok(ensurePool);
  assert.deepEqual(
    [10, 43, 47, 48, 49, 53, 74, 76].map((rel) => code.readUInt32LE(ensurePool.pc_start + rel * 4)),
    [
      ivmWord(0x20, 9, 10, 0),
      ivmWord(0x20, 24, 10, 0),
      ivmWord(0x20, 9, 10, 0),
      ivmWord(0x20, 24, 0, 0),
      ivmWord(0x0f, 23, 9, 24),
      ivmWord(0x20, 24, 0, 21),
      ivmWord(0x41, 23, 0, 2),
      ivmWord(0x20, 23, 0, 0),
    ],
  );
  assert.ok(initPoolImpl);
  assert.deepEqual(
    [26, 27, 52, 58, 91, 95, 96, 97, 98].map((rel) => code.readUInt32LE(initPoolImpl.pc_start + rel * 4)),
    [
      ivmWord(0x20, 6, 10, 0),
      ivmWord(0x23, 5, 6, 1),
      ivmWord(0x41, 5, 0, 2),
      ivmWord(0x20, 6, 10, 0),
      ivmWord(0x20, 4, 10, 0),
      ivmWord(0x20, 6, 10, 0),
      ivmWord(0x20, 4, 0, 0),
      ivmWord(0x0f, 5, 6, 4),
      ivmWord(0x23, 4, 5, 1),
    ],
  );
  assert.deepEqual(
    [129, 155, 162, 166, 167, 168, 426, 430, 456, 463, 464, 466, 467, 468].map((rel) => (
      code.readUInt32LE(initPoolImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x20, 5, 10, 0),
      ivmWord(0x20, 11, 5, 0),
      ivmWord(0x20, 6, 10, 0),
      ivmWord(0x20, 5, 10, 0),
      ivmWord(0x20, 10, 6, 0),
      ivmWord(0x20, 11, 5, 0),
      ivmWord(0x20, 9, 0, 0),
      ivmWord(0x20, 7, 10, 0),
      ivmWord(0x20, 11, 7, 0),
      ivmWord(0x20, 24, 10, 0),
      ivmWord(0x20, 10, 9, 0),
      ivmWord(0x20, 7, 10, 0),
      ivmWord(0x20, 10, 24, 0),
      ivmWord(0x20, 11, 7, 0),
    ],
  );
  assert.ok(depositLiquidityImpl);
  assert.deepEqual(
    [41, 42, 43, 44, 50, 51, 53, 54, 55, 81, 82, 83, 84, 86, 87, 88, 112].map((rel) => (
      code.readUInt32LE(depositLiquidityImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x20, 9, 31, 0),
      ivmWord(0x20, 9, 9, 127),
      ivmWord(0x20, 9, 9, 25),
      ivmWord(0x30, 9, 9, 0),
      ivmWord(0x20, 7, 0, 0),
      ivmWord(0x02, 12, 7, 9),
      ivmWord(0x08, 6, 12, 13),
      ivmWord(0x21, 6, 6, 1),
      ivmWord(0x20, 7, 0, 0),
      ivmWord(0x20, 6, 0, 0),
      ivmWord(0x20, 6, 0, 0),
      ivmWord(0x30, 27, 31, 40),
      ivmWord(0x02, 12, 6, 27),
      ivmWord(0x08, 7, 12, 13),
      ivmWord(0x21, 7, 7, 1),
      ivmWord(0x20, 6, 0, 0),
      ivmWord(0x41, 7, 0, 2),
    ],
  );
  assert.deepEqual(
    [142, 143, 144, 147, 173, 180, 184, 190].map((rel) => (
      code.readUInt32LE(depositLiquidityImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x20, 7, 0, 0),
      ivmWord(0x30, 27, 31, 8),
      ivmWord(0x20, 10, 27, 0),
      ivmWord(0x20, 6, 10, 0),
      ivmWord(0x20, 11, 6, 0),
      ivmWord(0x20, 5, 10, 0),
      ivmWord(0x20, 6, 10, 0),
      ivmWord(0x20, 5, 10, 0),
    ],
  );
  assert.deepEqual(
    [335, 338, 339, 340, 358, 359, 360].map((rel) => (
      code.readUInt32LE(depositLiquidityImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x20, 10, 0, 0),
      ivmWord(0x20, 10, 0, 25),
      ivmWord(0x06, 10, 10, 26),
      ivmWord(0x20, 10, 10, 41),
      ivmWord(0x20, 10, 10, 0),
      ivmWord(0x30, 27, 31, 8),
      ivmWord(0x20, 11, 27, 0),
    ],
  );
  assert.deepEqual(
    [361, 362, 363, 364, 413, 414, 415, 416, 466, 467, 468, 469].map((rel) => (
      code.readUInt32LE(depositLiquidityImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x46, 1, 0xfb, 0x7a),
      ivmWord(0x46, 0, 0, 0x18),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x46, 1, 0xfb, 0x46),
      ivmWord(0x46, 0, 0, 0x18),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x46, 1, 0xfb, 0x11),
      ivmWord(0x46, 0, 0, 0x18),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x20, 0, 0, 0),
    ],
  );
  assert.deepEqual(
    [616, 620, 621, 625, 626, 627, 646, 648, 651, 655].map((rel) => (
      code.readUInt32LE(depositLiquidityImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x30, 27, 31, 32),
      ivmWord(0x21, 23, 23, 1),
      ivmWord(0x20, 8, 0, 0),
      ivmWord(0x06, 8, 8, 26),
      ivmWord(0x20, 8, 8, 86),
      ivmWord(0x20, 8, 8, 0),
      ivmWord(0x60, 0, 0, 2),
      ivmWord(0x20, 10, 9, 0),
      ivmWord(0x20, 10, 24, 0),
      ivmWord(0x60, 0, 0, 224),
    ],
  );
  assert.deepEqual(
    [
      386, 387, 388, 389, 390, 391, 392, 393, 394,
      411, 412, 413, 438, 439, 466, 467,
      491, 492, 493, 494, 495, 496, 497, 498, 499, 500, 501, 522, 526,
      547, 548, 549, 550, 551, 552, 553, 554, 555, 556,
      578, 579, 580, 581, 582, 583, 584, 585, 586, 587, 588, 589, 590,
      615, 616, 617,
      814, 815, 816,
    ].map((rel) => (
      code.readUInt32LE(depositLiquidityImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x20, 6, 10, 0),
      ivmWord(0x20, 10, 0, 0),
      ivmWord(0x20, 26, 0, 0),
      ivmWord(0x20, 26, 0, 7),
      ivmWord(0x20, 10, 0, 26),
      ivmWord(0x06, 10, 10, 26),
      ivmWord(0x20, 10, 10, 1),
      ivmWord(0x20, 10, 10, 0),
      ivmWord(0x20, 10, 10, 0),
      ivmWord(0x30, 27, 31, 8),
      ivmWord(0x20, 11, 27, 0),
      ivmWord(0x46, 1, 0xfb, 0x46),
      ivmWord(0x20, 29, 10, 0),
      ivmWord(0x31, 31, 29, 24),
      ivmWord(0x46, 1, 0xfb, 0x11),
      ivmWord(0x46, 0, 0, 0x18),
      ivmWord(0x20, 29, 10, 0),
      ivmWord(0x31, 31, 29, 16),
      ivmWord(0x20, 7, 0, 0),
      ivmWord(0x30, 27, 31, 16),
      ivmWord(0x0e, 23, 27, 7),
      ivmWord(0x41, 23, 0, 26),
      ivmWord(0x46, 0, 0, 81),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x46, 0, 0, 25),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x30, 28, 31, 40),
      ivmWord(0x10, 23, 9, 28),
      ivmWord(0x1d, 7, 23, 0),
      ivmWord(0x20, 23, 0, 0),
      ivmWord(0x20, 29, 7, 0),
      ivmWord(0x31, 31, 29, 32),
      ivmWord(0x46, 0, 0, 62),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x30, 28, 31, 16),
      ivmWord(0x10, 23, 9, 28),
      ivmWord(0x14, 7, 23, 6),
      ivmWord(0x30, 27, 31, 40),
      ivmWord(0x30, 28, 31, 16),
      ivmWord(0x10, 23, 27, 28),
      ivmWord(0x30, 28, 31, 24),
      ivmWord(0x14, 8, 23, 28),
      ivmWord(0x1e, 23, 7, 8),
      ivmWord(0x20, 8, 0, 0),
      ivmWord(0x20, 29, 23, 0),
      ivmWord(0x31, 31, 29, 32),
      ivmWord(0x46, 0, 0, 25),
      ivmWord(0x20, 8, 0, 0),
      ivmWord(0x30, 27, 31, 32),
      ivmWord(0x02, 12, 8, 27),
      ivmWord(0x30, 28, 31, 32),
      ivmWord(0x01, 23, 27, 28),
      ivmWord(0x30, 27, 31, 8),
    ],
  );
  assert.deepEqual(
    [820, 846, 853, 854, 856, 857, 858].map((rel) => code.readUInt32LE(depositLiquidityImpl.pc_start + rel * 4)),
    [
      ivmWord(0x20, 9, 10, 0),
      ivmWord(0x20, 11, 9, 0),
      ivmWord(0x20, 2, 10, 0),
      ivmWord(0x20, 10, 23, 0),
      ivmWord(0x20, 9, 10, 0),
      ivmWord(0x20, 10, 2, 0),
      ivmWord(0x20, 11, 9, 0),
    ],
  );
  assert.ok(withdrawLiquidityImpl);
  assert.deepEqual(
    [93, 94, 95, 286, 289, 290, 310, 311, 312, 313, 338].map((rel) => (
      code.readUInt32LE(withdrawLiquidityImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x20, 7, 0, 0),
      ivmWord(0x30, 27, 31, 8),
      ivmWord(0x20, 10, 27, 0),
      ivmWord(0x20, 10, 0, 0),
      ivmWord(0x20, 10, 0, 25),
      ivmWord(0x06, 10, 10, 26),
      ivmWord(0x30, 27, 31, 8),
      ivmWord(0x20, 11, 27, 0),
      ivmWord(0x46, 1, 0xf7, 0x6f),
      ivmWord(0x46, 0, 0, 0x18),
      ivmWord(0x20, 10, 0, 0),
    ],
  );
  assert.deepEqual(
    [362, 363, 364, 365, 366, 367, 368, 389, 390, 391, 392, 393, 394, 395, 396].map((rel) => (
      code.readUInt32LE(withdrawLiquidityImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x30, 27, 31, 8),
      ivmWord(0x20, 11, 27, 0),
      ivmWord(0x46, 1, 0xf7, 0x3b),
      ivmWord(0x46, 0, 0, 0x18),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x20, 2, 10, 0),
      ivmWord(0x20, 10, 0, 0),
      ivmWord(0x20, 26, 0, 0),
      ivmWord(0x20, 26, 0, 7),
      ivmWord(0x20, 10, 0, 26),
      ivmWord(0x06, 10, 10, 26),
      ivmWord(0x20, 10, 10, 89),
      ivmWord(0x20, 10, 10, 0),
    ],
  );
  assert.deepEqual(
    [414, 415, 416, 417, 418, 419, 420].map((rel) => (
      code.readUInt32LE(withdrawLiquidityImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x30, 27, 31, 8),
      ivmWord(0x20, 11, 27, 0),
      ivmWord(0x46, 1, 0xf7, 0x07),
      ivmWord(0x46, 0, 0, 0x18),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x20, 0, 0, 0),
      ivmWord(0x20, 0, 0, 0),
    ],
  );
  assert.deepEqual(
    [479, 480, 481, 482, 483, 484, 485].map((rel) => (
      code.readUInt32LE(withdrawLiquidityImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x14, 7, 23, 28),
      ivmWord(0x30, 27, 31, 24),
      ivmWord(0x10, 23, 27, 2),
      ivmWord(0x30, 28, 31, 16),
      ivmWord(0x14, 29, 23, 28),
      ivmWord(0x31, 31, 29, 32),
      ivmWord(0x20, 23, 0, 0),
    ],
  );
  assert.deepEqual(
    [494, 495, 496, 497, 513, 527, 528, 529, 530, 546].map((rel) => (
      code.readUInt32LE(withdrawLiquidityImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x06, 23, 23, 26),
      ivmWord(0x20, 23, 23, 110),
      ivmWord(0x20, 23, 23, 0),
      ivmWord(0x20, 23, 23, 0),
      ivmWord(0x20, 23, 23, 0),
      ivmWord(0x06, 9, 9, 26),
      ivmWord(0x20, 9, 9, 31),
      ivmWord(0x20, 9, 9, 0),
      ivmWord(0x20, 9, 9, 0),
      ivmWord(0x20, 9, 9, 0),
    ],
  );
  assert.deepEqual(
    [721, 747, 754, 755, 757, 758, 759].map((rel) => (
      code.readUInt32LE(withdrawLiquidityImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x20, 8, 10, 0),
      ivmWord(0x20, 11, 8, 0),
      ivmWord(0x20, 2, 10, 0),
      ivmWord(0x20, 10, 4, 0),
      ivmWord(0x20, 8, 10, 0),
      ivmWord(0x20, 10, 2, 0),
      ivmWord(0x20, 11, 8, 0),
    ],
  );
  assert.ok(swapImpl);
  assert.deepEqual(
    [106, 153, 154, 158, 191, 192, 201, 238, 239, 248, 249, 274, 275, 300, 325, 326, 351, 359, 366, 367, 395, 500, 632, 634, 635, 636, 696, 893, 1065, 1091].map((rel) => (
      code.readUInt32LE(swapImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x20, 5, 0, 0),
      ivmWord(0x20, 29, 10, 0),
      ivmWord(0x31, 31, 29, 16),
      ivmWord(0x20, 5, 10, 0),
      ivmWord(0x20, 3, 10, 0),
      ivmWord(0x20, 10, 3, 0),
      ivmWord(0x20, 3, 10, 0),
      ivmWord(0x20, 2, 10, 0),
      ivmWord(0x20, 10, 2, 0),
      ivmWord(0x20, 29, 10, 0),
      ivmWord(0x31, 31, 29, 8),
      ivmWord(0x20, 11, 23, 0),
      ivmWord(0x46, 1, 0xf3, 0xe1),
      ivmWord(0x20, 5, 10, 0),
      ivmWord(0x20, 11, 23, 0),
      ivmWord(0x46, 1, 0xf3, 0xae),
      ivmWord(0x20, 2, 10, 0),
      ivmWord(0x20, 6, 10, 0),
      ivmWord(0x20, 4, 10, 0),
      ivmWord(0x04, 9, 6, 4),
      ivmWord(0x41, 6, 0, 26),
      ivmWord(0x41, 6, 0, 26),
      ivmWord(0x02, 12, 2, 7),
      ivmWord(0x08, 5, 12, 13),
      ivmWord(0x21, 5, 5, 1),
      ivmWord(0x23, 5, 5, 1),
      ivmWord(0x41, 6, 0, 26),
      ivmWord(0x20, 6, 0, 0),
      ivmWord(0x20, 5, 0, 0),
      ivmWord(0x20, 10, 2, 0),
    ],
  );
  assert.deepEqual(
    [446, 447, 473, 474, 551, 552, 578, 579, 607, 608, 615, 616, 619, 627, 628, 629, 630, 631].map((rel) => (
      code.readUInt32LE(swapImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x20, 9, 0, 0),
      ivmWord(0x20, 4, 5, 0),
      ivmWord(0x20, 9, 0, 0),
      ivmWord(0x20, 4, 2, 0),
      ivmWord(0x20, 9, 0, 0),
      ivmWord(0x20, 24, 2, 0),
      ivmWord(0x20, 9, 0, 0),
      ivmWord(0x20, 24, 5, 0),
      ivmWord(0x20, 9, 0, 0),
      ivmWord(0x20, 9, 9, 127),
      ivmWord(0x20, 9, 9, 108),
      ivmWord(0x10, 5, 8, 9),
      ivmWord(0x20, 9, 0, 0),
      ivmWord(0x20, 9, 9, 111),
      ivmWord(0x14, 2, 5, 9),
      ivmWord(0x10, 9, 24, 2),
      ivmWord(0x01, 5, 4, 2),
      ivmWord(0x14, 2, 9, 5),
    ],
  );
  assert.deepEqual(
    [352, 353, 354, 355, 356, 956, 960, 1006, 1010].map((rel) => (
      code.readUInt32LE(swapImpl.pc_start + rel * 4)
    )),
    [
      ivmWord(0x20, 10, 9, 0),
      ivmWord(0x60, 0, 0, 0xe0),
      ivmWord(0x20, 11, 10, 0),
      ivmWord(0x30, 28, 31, 16),
      ivmWord(0x20, 10, 28, 0),
      ivmWord(0x20, 5, 10, 0),
      ivmWord(0x20, 10, 5, 0),
      ivmWord(0x20, 23, 10, 0),
      ivmWord(0x20, 10, 23, 0),
    ],
  );
  assert.deepEqual(
    Array.from({ length: 24 }, (_, index) => code.readUInt32LE(swapImpl.pc_start + (637 + index) * 4)),
    [
      ivmWord(0x20, 7, 0, 0),
      ivmWord(0x20, 26, 0, 0),
      ivmWord(0x20, 26, 0, 7),
      ivmWord(0x20, 7, 0, 34),
      ivmWord(0x06, 7, 7, 26),
      ivmWord(0x20, 7, 7, 22),
      ...Array.from({ length: 18 }, () => ivmWord(0x20, 7, 7, 0)),
    ],
  );
  assert.deepEqual(
    [919, 923, 959, 961, 969].map((rel) => code.readUInt32LE(swapImpl.pc_start + rel * 4)),
    [
      ivmWord(0x01, 6, 4, 8),
      ivmWord(0x20, 7, 10, 0),
      ivmWord(0x20, 7, 10, 0),
      ivmWord(0x20, 11, 7, 0),
      ivmWord(0x02, 7, 24, 2),
    ],
  );
});

test("Kotodama compiler SDK keeps PredictionMarket direct account intervals Rust-shaped", () => {
  const source = readFileSync(
    new URL("../../../demo/prediction_market.ko", import.meta.url),
    "utf8",
  );
  const compiled = compileKotodamaProgram(source, { sourceName: "prediction_market.ko" });
  const code = readArtifactCode(compiled.artifactBytes);

  assert.deepEqual(compiled.diagnostics, []);
  assert.deepEqual(
    compiled.budgetReport.map((entry) => ([
      entry.function_name,
      entry.pc_start,
      entry.pc_end,
      entry.bytecode_bytes,
      entry.bytecode_words,
      entry.frame_bytes,
    ])),
    [
      ["main", 0, 6992, 6992, 1748, 88],
      ["hajimari", 6992, 7564, 572, 143, 24],
    ],
  );
  assert.deepEqual(
    Array.from({ length: 2 }, (_, index) => code.readUInt32LE((32 + index) * 4)),
    [
      ivmWord(0x20, 4, 10, 0),
      ivmWord(0x41, 4, 0, 26),
    ],
  );
  assert.deepEqual(
    Array.from({ length: 10 }, (_, index) => code.readUInt32LE((108 + index) * 4)),
    [
      ivmWord(0x60, 0, 0, 0xe0),
      ivmWord(0x60, 0, 0, 0x50),
      ivmWord(0x20, 6, 10, 0),
      ivmWord(0x20, 10, 6, 0),
      ivmWord(0x60, 0, 0, 0xe0),
      ivmWord(0x60, 0, 0, 0x53),
      ivmWord(0x20, 4, 10, 0),
      ivmWord(0x20, 6, 0, 0),
      ivmWord(0x0f, 3, 4, 6),
      ivmWord(0x23, 6, 3, 1),
    ],
  );
  assert.deepEqual(
    Array.from({ length: 3 }, (_, index) => code.readUInt32LE((237 + index) * 4)),
    [
      ivmWord(0x20, 6, 0, 0),
      ivmWord(0x20, 6, 6, 20),
      ivmWord(0x20, 10, 6, 0),
    ],
  );
  assert.deepEqual(
    Array.from({ length: 5 }, (_, index) => code.readUInt32LE((329 + index) * 4)),
    [
      ivmWord(0x20, 3, 0, 0),
      ivmWord(0x20, 3, 3, 1),
      ivmWord(0x20, 10, 3, 0),
      ivmWord(0x60, 0, 0, 0x55),
      ivmWord(0x20, 6, 10, 0),
    ],
  );
  assert.deepEqual(
    Array.from({ length: 2 }, (_, index) => code.readUInt32LE((496 + index) * 4)),
    [
      ivmWord(0x20, 3, 10, 0),
      ivmWord(0x41, 3, 0, 26),
    ],
  );
  assert.deepEqual(
    Array.from({ length: 9 }, (_, index) => code.readUInt32LE((642 + index) * 4)),
    [
      ivmWord(0x20, 5, 10, 0),
      ivmWord(0x20, 10, 5, 0),
      ivmWord(0x60, 0, 0, 0xe0),
      ivmWord(0x60, 0, 0, 0x53),
      ivmWord(0x20, 4, 10, 0),
      ivmWord(0x20, 5, 0, 0),
      ivmWord(0x0f, 6, 4, 5),
      ivmWord(0x03, 5, 3, 6),
      ivmWord(0x41, 5, 0, 26),
    ],
  );
  assert.deepEqual(
    Array.from({ length: 2 }, (_, index) => code.readUInt32LE((960 + index) * 4)),
    [
      ivmWord(0x20, 6, 10, 0),
      ivmWord(0x41, 6, 0, 26),
    ],
  );
  assert.deepEqual(
    Array.from({ length: 2 }, (_, index) => code.readUInt32LE((1080 + index) * 4)),
    [
      ivmWord(0x03, 3, 8, 6),
      ivmWord(0x41, 3, 0, 26),
    ],
  );
  assert.deepEqual(
    Array.from({ length: 2 }, (_, index) => code.readUInt32LE((1368 + index) * 4)),
    [
      ivmWord(0x20, 6, 10, 0),
      ivmWord(0x41, 6, 0, 26),
    ],
  );
});

test("Kotodama parity source normalization is exported by the SDK", () => {
  assert.equal(normalizeKotodamaParitySource("one\r\ntwo\r\n"), "one\ntwo\n");
});
