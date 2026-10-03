// Complete V1 callable type tapes. Public record limits remain independently
// enforced by entrypointSchema.js; private callable forests have their own bound.
import { isCanonicalKotodamaIdentifier, isCanonicalKotodamaStructName } from "../kotodamaIdentifiers.js";
import { normalizeContractErrorTypeV1 } from "../contractErrorTypes.js";
import { readU32Le, readU64Le, readCompactField, decodeEmbeddedString, visitEmbeddedVector } from "./embeddedNorito.js";

const MAX_NODES = 250_000;
const MAX_DEPTH = 256;
const MAX_WORDS = 8192;
const INTERNAL_POINTERS = new Set([0x0b, 0x0d, 0x0e, 0x0f, 0x13]);
const CORE_VIEWS = new Map([
  ["AccountView", [["id", "metadata"], [7, 5]]],
  ["AssetView", [["id", "amount"], [9, 2]]],
  ["AssetDefinitionView", [["id", "name", "description", "owned_by", "total_quantity", "numeric_scale", "metadata"], [8, 4, "option", 4, 7, 2, "option", 0, 5]]],
  ["DomainView", [["id", "owned_by", "metadata"], [10, 7, 5]]],
  ["NftView", [["id", "owned_by", "content"], [11, 7, 5]]],
]);

function fail(label, reason) { throw new TypeError(`${label} ${reason}`); }
function fields(bytes, count, label, offset = 0) {
  const state = { offset };
  const result = Array.from({ length: count }, (_, index) => readCompactField(bytes, state, `${label}.field${index}`));
  if (state.offset !== bytes.length) fail(label, "has trailing or missing fields");
  return result;
}
function integer(bytes, width, label) {
  if (bytes.length !== width) fail(label, `requires an exact ${width}-byte integer`);
  if (width === 4) return readU32Le(bytes, 0, label);
  return bytes[0] + (width === 2 ? bytes[1] * 256 : 0);
}
function strings(bytes, maximum, label) {
  const result = [];
  visitEmbeddedVector(bytes, label, maximum, (item, itemLabel) => result.push(decodeEmbeddedString(item, itemLabel)));
  return result;
}
function nominalError(bytes, label, catalog) {
  const [identityBytes, variantsBytes] = fields(bytes, 2, label);
  const variants = [];
  visitEmbeddedVector(variantsBytes, `${label}.variants`, 256, (bytes, variantLabel) => {
    const [name, code] = fields(bytes, 2, variantLabel);
    variants.push({ name: decodeEmbeddedString(name, variantLabel), code: integer(code, 4, variantLabel) });
  });
  const descriptor = normalizeContractErrorTypeV1({ identity: decodeEmbeddedString(identityBytes, label), variants }, label);
  if (catalog.get(descriptor.identity) !== JSON.stringify(descriptor)) fail(label, "does not match its declared nominal error catalog");
}
function node(bytes, label, zk, catalog) {
  const kind = readU32Le(bytes, 0, label);
  const result = { kind, children: 0, resource: false };
  if ([2, 3, 6, 9].includes(kind)) {
    fields(bytes, 0, label, 4);
    result.children = kind === 2 ? 1 : kind === 3 ? 2 : 0;
    result.resource = kind === 9;
    return result;
  }
  if (kind === 0) {
    const [name, names] = fields(bytes, 2, label, 4);
    result.name = decodeEmbeddedString(name, label);
    result.fields = strings(names, MAX_NODES, label);
    const reserved = CORE_VIEWS.has(result.name) || result.name === "QueryPage" || result.name === "StatePage";
    if ((!reserved && !isCanonicalKotodamaStructName(result.name)) ||
        result.fields.some((name) => !isCanonicalKotodamaIdentifier(name)) ||
        new Set(result.fields).size !== result.fields.length) fail(label, "has a noncanonical nominal product");
    result.children = result.fields.length;
    return result;
  }
  const [payload] = fields(bytes, 1, label, 4);
  switch (kind) {
    case 1:
      result.children = integer(payload, 4, label);
      if (result.children < 2) fail(label, "requires tuple arity of at least two");
      break;
    case 4:
      result.capacity = integer(payload, 1, label);
      if (result.capacity < 1 || result.capacity > 64) fail(label, "requires List capacity in 1..64");
      result.children = 1;
      break;
    case 5:
    case 8:
      result.leaf = integer(payload, 4, label);
      if (result.leaf > 13 || (kind === 8 && result.leaf === 5)) fail(label, "has an invalid scalar or cursor key kind");
      break;
    case 7:
      nominalError(payload, label, catalog);
      break;
    case 10:
    case 11:
      result.pointer = integer(payload, 2, label);
      if (kind === 10 ? !INTERNAL_POINTERS.has(result.pointer) : !zk || result.pointer < 0x10 || result.pointer > 0x12) {
        fail(label, "has an invalid internal pointer or private numeric type");
      }
      result.resource = kind === 11 || result.pointer === 0x13;
      break;
    default:
      fail(label, "has an unknown callable schema node");
  }
  return result;
}

function sameNames(actual, expected) {
  return actual.length === expected.length && actual.every((value, index) => value === expected[index]);
}
function coreView(nodes, start, ends) {
  const root = nodes[start];
  const shape = CORE_VIEWS.get(root?.name);
  if (root?.kind !== 0 || !shape || !sameNames(root.fields, shape[0]) || ends[start] !== start + shape[1].length + 1) return false;
  return shape[1].every((expected, offset) => {
    const actual = nodes[start + offset + 1];
    return expected === "option" ? actual?.kind === 2 : actual?.kind === 5 && actual.leaf === expected;
  });
}
function reservedShapes(nodes, ends, label) {
  nodes.forEach((root, start) => {
    if (root.kind !== 0) return;
    if (CORE_VIEWS.has(root.name)) {
      if (!coreView(nodes, start, ends)) fail(label, "contains a forged reserved query view");
    } else if (root.name === "QueryPage") {
      const after = ends[start + 2];
      if (!sameNames(root.fields, ["items", "next_offset"]) || nodes[start + 1]?.kind !== 4 || nodes[start + 1].capacity !== 64 ||
          !coreView(nodes, start + 2, ends) || nodes[after]?.kind !== 2 || nodes[after + 1]?.kind !== 5 || nodes[after + 1].leaf !== 0 || ends[start] !== after + 2) {
        fail(label, "contains a forged QueryPage");
      }
    } else if (root.name === "StatePage") {
      const after = ends[start + 4];
      const key = nodes[start + 3];
      if (!sameNames(root.fields, ["items", "next"]) || nodes[start + 1]?.kind !== 4 || nodes[start + 2]?.kind !== 1 || nodes[start + 2].children !== 2 ||
          key?.kind !== 5 || key.leaf === 5 || nodes[after]?.kind !== 2 || nodes[after + 1]?.kind !== 8 || nodes[after + 1].leaf !== key.leaf || ends[start] !== after + 2) {
        fail(label, "contains a forged StatePage");
      }
    }
  });
}

function schema(bytes, label, zk, catalog) {
  const [tape] = fields(bytes, 1, label);
  const nodes = [];
  visitEmbeddedVector(tape, `${label}.nodes`, MAX_NODES, (bytes, itemLabel) => nodes.push(node(bytes, itemLabel, zk, catalog)));
  const ends = new Uint32Array(nodes.length);
  const stack = [];
  let roots = 0;
  let words = 0;
  nodes.forEach((current, index) => {
    if (stack.length + 1 > MAX_DEPTH || current.children > nodes.length - index - 1) fail(label, "has an incomplete or over-depth callable type forest");
    if (current.children !== 0) {
      stack.push({ index, remaining: current.children, words: 0, resource: false });
      return;
    }
    let finished = index;
    let width = 1;
    let resource = current.resource;
    for (;;) {
      ends[finished] = index + 1;
      if (stack.length === 0) {
        roots += 1;
        words += width;
        if (words > MAX_WORDS) fail(label, "exceeds the 8192-word call table bound");
        break;
      }
      const parent = stack[stack.length - 1];
      parent.words += width;
      parent.resource ||= resource;
      parent.remaining -= 1;
      if (parent.remaining !== 0) break;
      stack.pop();
      finished = parent.index;
      resource = parent.resource;
      const kind = nodes[finished].kind;
      if (kind === 4 && resource) fail(label, "cannot place a private or affine resource inside a List");
      width = [2, 3, 4].includes(kind) ? 1 : parent.words;
      // Every internal width is at most MAX_NODES. Even the largest List
      // footprint is <= (2 + 64 * MAX_NODES) * 8 and fits the native u64 bound.
    }
  });
  if (stack.length !== 0) fail(label, "ends before its callable type forest is complete");
  reservedShapes(nodes, ends, label);
  return { roots, words };
}

export function validateEmbeddedCallables(bytes, headerMode, minimumCount, label, errorTypes) {
  const catalog = new Map(errorTypes.map((value) => {
    const descriptor = normalizeContractErrorTypeV1(value);
    return [descriptor.identity, JSON.stringify(descriptor)];
  }));
  let lastEntryPc = -1n;
  const count = visitEmbeddedVector(bytes, label, 65_536, (bytes, itemLabel) => {
    const [entry, frame, argumentsBytes, resultsBytes] = fields(bytes, 4, itemLabel);
    if (entry.length !== 8) fail(itemLabel, "requires an exact u64 entry offset");
    const entryPc = readU64Le(entry, 0, itemLabel);
    const frameBytes = integer(frame, 4, itemLabel);
    if (entryPc <= lastEntryPc || entryPc % 4n !== 0n || frameBytes % 16 !== 0 || frameBytes > 4 * 1024 * 1024) fail(itemLabel, "requires ordered aligned roots and bounded aligned frames");
    lastEntryPc = entryPc;
    schema(argumentsBytes, `${itemLabel}.arguments`, (headerMode & 1) !== 0, catalog);
    const results = schema(resultsBytes, `${itemLabel}.results`, (headerMode & 1) !== 0, catalog);
    if (results.roots !== 1 || results.words === 0) fail(itemLabel, "requires one nonempty result type tree");
  });
  if (count < minimumCount) fail(label, "must cover every public entrypoint");
  return lastEntryPc;
}
