import { isCanonicalKotodamaIdentifier } from "./kotodamaIdentifiers.js";

const MAX_ENTRYPOINT_TYPE_NODES_V1 = 256;
const MAX_ENTRYPOINT_TYPE_DEPTH_V1 = 256;
const MIN_ENTRYPOINT_LIST_CAPACITY_V1 = 1;
const MAX_ENTRYPOINT_LIST_CAPACITY_V1 = 64;

const LEAF_TYPE_NAMES = new Map([
  ["Int", "int"],
  ["Decimal", "decimal"],
  ["Quantity", "quantity"],
  ["Bool", "bool"],
  ["String", "string"],
  ["Json", "Json"],
  ["Name", "Name"],
  ["AccountId", "AccountId"],
  ["AssetDefinitionId", "AssetDefinitionId"],
  ["AssetId", "AssetId"],
  ["DomainId", "DomainId"],
  ["NftId", "NftId"],
  ["DataSpaceId", "DataSpaceId"],
  ["Blob", "bytes"],
]);

const CORE_QUERY_VIEWS = new Map([
  ["AccountView", { fields: ["id", "metadata"], children: ["AccountId", "Json"] }],
  ["AssetView", { fields: ["id", "amount"], children: ["AssetId", "quantity"] }],
  [
    "AssetDefinitionView",
    {
      fields: ["id", "name", "description", "owned_by", "total_quantity", "metadata"],
      children: [
        "AssetDefinitionId",
        "string",
        "Option<string>",
        "AccountId",
        "quantity",
        "Json",
      ],
    },
  ],
  [
    "DomainView",
    { fields: ["id", "owned_by", "metadata"], children: ["DomainId", "AccountId", "Json"] },
  ],
  [
    "NftView",
    { fields: ["id", "owned_by", "content"], children: ["NftId", "AccountId", "Json"] },
  ],
]);

function fail(context, message) {
  throw new TypeError(`${context} ${message}`);
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function requireExactKeys(value, expected, context) {
  if (!isRecord(value)) {
    fail(context, "must be an object");
  }
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (
    actual.length !== wanted.length ||
    actual.some((entry, index) => entry !== wanted[index])
  ) {
    fail(context, `must contain exactly ${expected.join(" and ")}`);
  }
}

function normalizeUnsignedInteger(value, maximum, context) {
  let normalized;
  if (typeof value === "bigint") {
    normalized = value;
  } else if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) {
      fail(context, "must be a safe unsigned integer");
    }
    normalized = BigInt(value);
  } else if (typeof value === "string" && /^(?:0|[1-9][0-9]*)$/u.test(value)) {
    normalized = BigInt(value);
  } else {
    fail(context, "must be an unsigned integer");
  }
  if (normalized < 0n || normalized > BigInt(maximum)) {
    fail(context, `must be in 0..${maximum}`);
  }
  return Number(normalized);
}

function childCount(node, context) {
  switch (node.kind) {
    case "Struct":
      return node.value.fields.length;
    case "Tuple":
      return normalizeUnsignedInteger(node.value, 0xffff, `${context}.value`);
    case "Option":
    case "List":
      return 1;
    case "Result":
      return 2;
    case "Leaf":
      return 0;
    default:
      fail(`${context}.kind`, "is not a V1 entrypoint value-type node");
  }
}

function validateNode(node, context) {
  requireExactKeys(node, ["kind", "value"], context);
  if (typeof node.kind !== "string") {
    fail(`${context}.kind`, "must be a string");
  }
  switch (node.kind) {
    case "Struct": {
      requireExactKeys(node.value, ["name", "fields"], `${context}.value`);
      const reservedSchemaName =
        CORE_QUERY_VIEWS.has(node.value.name) || node.value.name === "QueryPage";
      if (
        (!reservedSchemaName &&
          !isCanonicalKotodamaIdentifier(node.value.name, { typeDeclaration: true })) ||
        !Array.isArray(node.value.fields) ||
        node.value.fields.length === 0
      ) {
        fail(context, "contains a noncanonical or empty struct descriptor");
      }
      const fields = new Set();
      for (const field of node.value.fields) {
        if (!isCanonicalKotodamaIdentifier(field) || fields.has(field)) {
          fail(context, "contains a duplicate or noncanonical struct field");
        }
        fields.add(field);
      }
      break;
    }
    case "Tuple": {
      const arity = normalizeUnsignedInteger(node.value, 0xffff, `${context}.value`);
      if (arity < 2) {
        fail(`${context}.value`, "must be in the V1 tuple range 2..65535");
      }
      break;
    }
    case "Option":
    case "Result":
      if (node.value !== null) {
        fail(`${context}.value`, "must be null");
      }
      break;
    case "List": {
      requireExactKeys(node.value, ["capacity"], `${context}.value`);
      const capacity = normalizeUnsignedInteger(
        node.value.capacity,
        0xff,
        `${context}.value.capacity`,
      );
      if (
        capacity < MIN_ENTRYPOINT_LIST_CAPACITY_V1 ||
        capacity > MAX_ENTRYPOINT_LIST_CAPACITY_V1
      ) {
        fail(`${context}.value.capacity`, "must be in the V1 range 1..64");
      }
      break;
    }
    case "Leaf": {
      requireExactKeys(node.value, ["kind", "value"], `${context}.value`);
      if (!LEAF_TYPE_NAMES.has(node.value.kind) || node.value.value !== null) {
        fail(`${context}.value`, "is not a canonical V1 entrypoint value kind");
      }
      break;
    }
    default:
      fail(`${context}.kind`, "is not a V1 entrypoint value-type node");
  }
}

/**
 * Validate and analyze one canonical flat-preorder Kotodama V1 boundary type.
 *
 * The returned canonical name is also used to bind manifest spelling to the
 * exact schema. Lists carry only their capacity; their single element subtree
 * is the next complete subtree in `nodes`.
 */
export function analyzeEntrypointValueTypeV1(value, context = "entrypoint value type") {
  requireExactKeys(value, ["nodes"], context);
  if (
    !Array.isArray(value.nodes) ||
    value.nodes.length === 0 ||
    value.nodes.length > MAX_ENTRYPOINT_TYPE_NODES_V1
  ) {
    fail(`${context}.nodes`, "must contain 1..256 canonical type nodes");
  }
  value.nodes.forEach((node, index) => validateNode(node, `${context}.nodes[${index}]`));

  const frames = [];
  let wordCount = 0;
  let maxDepth = 0;
  value.nodes.forEach((node, index) => {
    while (frames[frames.length - 1]?.remaining === 0) {
      frames.pop();
    }
    let suppressWords = false;
    if (index !== 0) {
      const parent = frames[frames.length - 1];
      if (parent === undefined || parent.remaining === 0) {
        fail(`${context}.nodes`, "is not one complete canonical prefix type tree");
      }
      parent.remaining -= 1;
      suppressWords = parent.suppressWords;
    }
    const depth = frames.length + 1;
    if (depth > MAX_ENTRYPOINT_TYPE_DEPTH_V1) {
      fail(context, "exceeds the V1 recursive type depth");
    }
    maxDepth = Math.max(maxDepth, depth);

    const handle = node.kind === "Option" || node.kind === "Result" || node.kind === "List";
    if (!suppressWords && (handle || node.kind === "Leaf")) {
      wordCount += 1;
    }
    const children = childCount(node, `${context}.nodes[${index}]`);
    if (children !== 0) {
      frames.push({
        remaining: children,
        suppressWords: suppressWords || handle,
      });
    }
  });
  while (frames[frames.length - 1]?.remaining === 0) {
    frames.pop();
  }
  if (frames.length !== 0) {
    fail(`${context}.nodes`, "is not one complete canonical prefix type tree");
  }

  const rendered = [];
  for (let index = value.nodes.length - 1; index >= 0; index -= 1) {
    const node = value.nodes[index];
    const children = childCount(node, `${context}.nodes[${index}]`);
    if (rendered.length < children) {
      fail(`${context}.nodes`, "ends before its prefix type tree is complete");
    }
    const childValues = rendered.splice(rendered.length - children, children).reverse();
    let result;
    switch (node.kind) {
      case "Struct": {
        const reserved = CORE_QUERY_VIEWS.get(node.value.name);
        if (reserved !== undefined) {
          if (
            JSON.stringify(node.value.fields) !== JSON.stringify(reserved.fields) ||
            JSON.stringify(childValues.map((child) => child.canonicalName)) !==
              JSON.stringify(reserved.children)
          ) {
            fail(context, "contains a forged reserved query-view schema");
          }
          result = { canonicalName: node.value.name, coreView: node.value.name };
        } else if (node.value.name === "QueryPage") {
          const [items, nextOffset] = childValues;
          if (
            JSON.stringify(node.value.fields) !== JSON.stringify(["items", "next_offset"]) ||
            items?.kind !== "List" ||
            items.capacity !== 64 ||
            items.listElementCoreView === undefined ||
            nextOffset?.canonicalName !== "Option<int>"
          ) {
            fail(context, "contains a forged QueryPage schema");
          }
          result = { canonicalName: `QueryPage<${items.listElementCoreView}>` };
        } else {
          result = { canonicalName: `struct ${node.value.name}` };
        }
        break;
      }
      case "Tuple":
        result = {
          canonicalName: `(${childValues.map((child) => child.canonicalName).join(", ")})`,
        };
        break;
      case "Option":
        result = { canonicalName: `Option<${childValues[0].canonicalName}>` };
        break;
      case "Result":
        result = {
          canonicalName: `Result<${childValues[0].canonicalName}, ${childValues[1].canonicalName}>`,
        };
        break;
      case "List":
        result = {
          canonicalName: `List<${childValues[0].canonicalName}, ${Number(node.value.capacity)}>`,
          kind: "List",
          capacity: Number(node.value.capacity),
          listElementCoreView: childValues[0].coreView,
        };
        break;
      case "Leaf":
        result = { canonicalName: LEAF_TYPE_NAMES.get(node.value.kind) };
        break;
      default:
        fail(context, "contains an unsupported V1 type node");
    }
    rendered.push(result);
  }
  if (rendered.length !== 1) {
    fail(`${context}.nodes`, "is not one complete canonical prefix type tree");
  }
  return {
    nodeCount: value.nodes.length,
    maxDepth,
    wordCount,
    canonicalName: rendered[0].canonicalName,
  };
}
