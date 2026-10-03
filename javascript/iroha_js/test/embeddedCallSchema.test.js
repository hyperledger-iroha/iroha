import assert from "node:assert/strict";
import test from "node:test";
import { validateEmbeddedCallables } from "../src/kotodamaCompiler/embeddedCallSchema.js";

const join = (...parts) => Buffer.concat(parts);
const u32 = (n) => { const out = Buffer.alloc(4); out.writeUInt32LE(n); return out; };
const u64 = (n) => { const out = Buffer.alloc(8); out.writeBigUInt64LE(BigInt(n)); return out; };
const field = (bytes) => {
  const prefix = [];
  let size = bytes.length;
  do { const low = size % 128; size = Math.floor(size / 128); prefix.push(low | (size ? 128 : 0)); } while (size);
  return join(Buffer.from(prefix), bytes);
};
const vector = (items) => join(u64(items.length), ...items.map(field));
const string = (value) => field(Buffer.from(value));
const unit = () => u32(6);
const leaf = (kind) => join(u32(5), field(u32(kind)));
const cursor = (kind) => join(u32(8), field(u32(kind)));
const tuple = (count) => join(u32(1), field(u32(count)));
const list = (capacity) => join(u32(4), field(Buffer.of(capacity)));
const pointer = (kind, id) => join(u32(kind), field(Buffer.of(id, 0)));
const product = (name, names) => join(u32(0), field(string(name)), field(vector(names.map(string))));
function callable(argumentsNodes = [], resultNodes = [unit()]) {
  return join(field(u64(0)), field(u32(0)), field(field(vector(argumentsNodes))), field(field(vector(resultNodes))));
}
function check(argumentsNodes = [], resultNodes = [unit()], mode = 0, errors = []) {
  return validateEmbeddedCallables(vector([callable(argumentsNodes, resultNodes)]), mode, 1, "callables", errors);
}

test("callable forests bind full scalar, internal and private types with exact word bounds", () => {
  assert.equal(check(Array.from({ length: 14 }, (_, kind) => leaf(kind))), 0n);
  for (const id of [0x0b, 0x0d, 0x0e, 0x0f, 0x13]) check([pointer(10, id)]);
  for (const id of [0x10, 0x11, 0x12]) {
    check([pointer(11, id)], [unit()], 1);
    assert.throws(() => check([pointer(11, id)]), /private/u);
  }
  assert.throws(() => check([pointer(10, 0x11)]), /pointer/u);
  assert.throws(() => check([pointer(11, 0x13)], [unit()], 1), /private/u);
  check([product("Empty", [])], [tuple(8192), ...Array.from({ length: 8192 }, unit)]);
  assert.throws(() => check(Array.from({ length: 8193 }, unit)), /8192/u);
  assert.throws(() => check([], []), /result type/u);
  assert.throws(() => check([], [unit(), unit()]), /result type/u);
  // A handle counts once even when its complete interior exceeds a table.
  check([u32(2), tuple(9000), ...Array.from({ length: 9000 }, unit)]);
});

test("callable schema traversal rejects incomplete, deep and retired layouts", () => {
  check([...Array.from({ length: 255 }, () => u32(2)), unit()]);
  assert.throws(() => check([...Array.from({ length: 256 }, () => u32(2)), unit()]), /depth/u);
  for (const nodes of [[u32(2)], [u32(3), unit()], [tuple(1), unit()], [list(0), unit()], [list(65), unit()], [leaf(14)], [cursor(5)], [u32(12)]]) {
    assert.throws(() => check(nodes), /callables/u);
  }
  const retired = join(field(u64(0)), field(u32(0)), field(vector([])), field(vector([u32(0)])));
  assert.throws(() => validateEmbeddedCallables(vector([retired]), 0, 1, "callables", []), /callables/u);
  const overNodes = join(field(u64(0)), field(u32(0)), field(field(u64(250001))), field(field(vector([unit()]))));
  assert.throws(() => validateEmbeddedCallables(vector([overNodes]), 0, 1, "callables", []), /250000/u);
});

test("List schemas reject affine resources through nested products and sums", () => {
  for (const resource of [u32(9), pointer(10, 0x13), pointer(11, 0x11)]) {
    assert.throws(() => check([list(64), u32(3), unit(), u32(2), resource], [unit()], 1), /resource/u);
  }
  check([list(64), u32(3), leaf(4), leaf(13)]);
});

test("reserved nominal schemas authenticate exact fields, view leaves and cursor keys", () => {
  const account = [product("AccountView", ["id", "metadata"]), leaf(7), leaf(5)];
  check(account);
  check([product("QueryPage", ["items", "next_offset"]), list(64), ...account, u32(2), leaf(0)]);
  assert.throws(() => check([product("AccountView", ["id", "metadata"]), leaf(7), leaf(13)]), /query view/u);
  assert.throws(() => check([product("QueryPage", ["items", "next_offset"]), list(63), ...account, u32(2), leaf(0)]), /QueryPage/u);
  const page = [product("StatePage", ["items", "next"]), list(4), tuple(2), leaf(4), u32(2), leaf(13), u32(2), cursor(4)];
  check(page);
  assert.throws(() => check([...page.slice(0, -1), cursor(13)]), /StatePage/u);
  assert.throws(() => check([product("Repeated", ["x", "x"]), unit(), unit()]), /nominal/u);
});

test("nominal error tapes retain the complete declared identity and variant catalog", () => {
  const descriptor = { identity: "Demo::Failure", variants: [{ name: "Bad", code: 1 }] };
  const encode = (code) => join(u32(7), field(join(field(string(descriptor.identity)), field(vector([join(field(string("Bad")), field(u32(code)))])))));
  check([encode(1)], [unit()], 0, [descriptor]);
  assert.throws(() => check([encode(2)], [unit()], 0, [descriptor]), /catalog/u);
  assert.throws(() => check([encode(1)]), /catalog/u);
  assert.throws(() => check([encode(0)], [unit()], 0, [descriptor]), /nonzero/u);
});
