const isArray = Array.isArray.bind(Array);
const stringify = JSON.stringify.bind(JSON);
const TEXT_MUST_BE = "must be ";
const TEXT_STATECURSOR = "StateCursor";
const TEXT_CONTAINS_A = "contains a ";
const TEXT_ACCOUNTID = "AccountId";
const TEXT_ASSETDEFINITIONID = "AssetDefinitionId";
const TEXT_OPTION = "Option";
const TEXT_NONCANONICAL_STRUCT = "noncanonical struct ";
const TEXT_ENTRYPOINT = "entrypoint ";
function rejectError(ErrorType, ...args) { throw new ErrorType(...args); }
import { normalizeContractErrorTypeV1 } from "./contractErrorTypes.js";
import { isCanonicalKotodamaIdentifier, isCanonicalKotodamaStructName } from "./kotodamaIdentifiers.js";

const TEXT_IS_NOT_ONE_COMPLETE_CANONICAL_PREFIX_TYPE_TREE = "is not one complete canonical prefix type tree";
const TEXT_IS_NOT_A_V1_ENTRYPOINT_VALUE_TYPE_NODE = ("is not a V1 " + TEXT_ENTRYPOINT + "value-type node");


/** Maximum initialized words in one V1 argument or result table. */
export const MAX_ENTRYPOINT_CALL_TABLE_WORDS_V1 = 8192;

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
  [(TEXT_ACCOUNTID), (TEXT_ACCOUNTID)],
  [(TEXT_ASSETDEFINITIONID), (TEXT_ASSETDEFINITIONID)],
  ["AssetId", "AssetId"],
  ["DomainId", "DomainId"],
  ["NftId", "NftId"],
  ["DataSpaceId", "DataSpaceId"],
  ["Blob", "bytes"],
]);

const CORE_QUERY_VIEWS = new Map([
  ["AccountView", { fields: ["id", "metadata"], children: [(TEXT_ACCOUNTID), "Json"] }],
  ["AssetView", { fields: ["id", "amount"], children: ["AssetId", "quantity"] }],
  [
    "AssetDefinitionView",
    {
      fields: ["id", "name", "description", "owned_by", "total_quantity", "numeric_scale", "metadata"],
      children: [
        (TEXT_ASSETDEFINITIONID),
        "string",
        (TEXT_OPTION + "<string>"),
        (TEXT_ACCOUNTID),
        "quantity",
        "Option<int>",
        "Json",
      ],
    },
  ],
  [
    "DomainView",
    { fields: ["id", "owned_by", "metadata"], children: ["DomainId", (TEXT_ACCOUNTID), "Json"] },
  ],
  [
    "NftView",
    { fields: ["id", "owned_by", "content"], children: ["NftId", (TEXT_ACCOUNTID), "Json"] },
  ],
]);

function fail(context, message) {
  rejectError(TypeError, `${context} ${message}`);
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !isArray(value);
}

function requireExactKeys(value, expected, context) {
  if (!isRecord(value)) {
    fail(context, (TEXT_MUST_BE + "an object"));
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
      fail(context, (TEXT_MUST_BE + "a safe unsigned integer"));
    }
    normalized = BigInt(value);
  } else if (typeof value === "string" && /^(?:0|[1-9][0-9]*)$/u.test(value)) {
    normalized = BigInt(value);
  } else {
    fail(context, (TEXT_MUST_BE + "an unsigned integer"));
  }
  if (normalized < 0n || normalized > BigInt(maximum)) {
    fail(context, `${TEXT_MUST_BE}in 0..${maximum}`);
  }
  return Number(normalized);
}

function childCount(node, context) {
  switch (node.kind) {
    case "Struct":
      return node.value.fields.length;
    case "Tuple":
      return normalizeUnsignedInteger(node.value, 0xffff, `${context}.value`);
    case (TEXT_OPTION):
    case "List":
      return 1;
    case "Result":
      return 2;
    case "Leaf":
    case "Unit":
    case "Error":
    case (TEXT_STATECURSOR):
      return 0;
    default:
      fail(`${context}.kind`, TEXT_IS_NOT_A_V1_ENTRYPOINT_VALUE_TYPE_NODE);
  }
}

function validateNode(node, context) {
  requireExactKeys(node, ["kind", "value"], context);
  if (typeof node.kind !== "string") {
    fail(`${context}.kind`, (TEXT_MUST_BE + "a string"));
  }
  switch (node.kind) {
    case "Struct": {
      requireExactKeys(node.value, ["name", "fields"], `${context}.value`);
      const reservedSchemaName =
        CORE_QUERY_VIEWS.has(node.value.name) || node.value.name === "QueryPage" || node.value.name === "StatePage";
      if (
        (!reservedSchemaName &&
          !isCanonicalKotodamaStructName(node.value.name)) ||
        !isArray(node.value.fields)
      ) {
        fail(context, (TEXT_CONTAINS_A + TEXT_NONCANONICAL_STRUCT + "descriptor"));
      }
      const fields = new Set();
      for (const field of node.value.fields) {
        if (!isCanonicalKotodamaIdentifier(field) || fields.has(field)) {
          fail(context, (TEXT_CONTAINS_A + "duplicate or " + TEXT_NONCANONICAL_STRUCT + "field"));
        }
        fields.add(field);
      }
      break;
    }
    case "Tuple": {
      const arity = normalizeUnsignedInteger(node.value, 0xffff, `${context}.value`);
      if (arity < 2) {
        fail(`${context}.value`, (TEXT_MUST_BE + "in the V1 tuple range 2..65535"));
      }
      break;
    }
    case "Unit":
    case (TEXT_OPTION):
    case "Result":
      if (node.value !== null) {
        fail(`${context}.value`, (TEXT_MUST_BE + "null"));
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
        fail(`${context}.value.capacity`, (TEXT_MUST_BE + "in the V1 range 1..64"));
      }
      break;
    }
    case "Error":
      normalizeContractErrorTypeV1(node.value, `${context}.value`);
      break;
    case (TEXT_STATECURSOR):
      if (node.value?.kind === "Json") fail(context, "cannot use Json cursor keys");
      // Cursor schemas carry one exact scalar key-kind descriptor.
    case "Leaf": {
      requireExactKeys(node.value, ["kind", "value"], `${context}.value`);
      if (!LEAF_TYPE_NAMES.has(node.value.kind) || node.value.value !== null) {
        fail(`${context}.value`, ("is not a canonical V1 " + TEXT_ENTRYPOINT + "value kind"));
      }
      break;
    }
    default:
      fail(`${context}.kind`, TEXT_IS_NOT_A_V1_ENTRYPOINT_VALUE_TYPE_NODE);
  }
}

/**
 * Validate and analyze one canonical flat-preorder Kotodama V1 boundary type.
 *
 * The returned canonical name is also used to bind manifest spelling to the
 * exact schema. Lists carry only their capacity; their single element subtree
 * is the next complete subtree in `nodes`.
 */
export function analyzeEntrypointValueTypeV1(value, context = (TEXT_ENTRYPOINT + "value type")) {
  requireExactKeys(value, ["nodes"], context);
  if (
    !isArray(value.nodes) ||
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
        fail(`${context}.nodes`, TEXT_IS_NOT_ONE_COMPLETE_CANONICAL_PREFIX_TYPE_TREE);
      }
      parent.remaining -= 1;
      suppressWords = parent.suppressWords;
    }
    const depth = frames.length + 1;
    if (depth > MAX_ENTRYPOINT_TYPE_DEPTH_V1) {
      fail(context, "exceeds the V1 recursive type depth");
    }
    maxDepth = Math.max(maxDepth, depth);

    const handle = node.kind === (TEXT_OPTION) || node.kind === "Result" || node.kind === "List";
    const children = childCount(node, `${context}.nodes[${index}]`);
    if (!suppressWords && (handle || children === 0)) {
      wordCount += 1;
    }
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
    fail(`${context}.nodes`, TEXT_IS_NOT_ONE_COMPLETE_CANONICAL_PREFIX_TYPE_TREE);
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
            stringify(node.value.fields) !== stringify(reserved.fields) ||
            stringify(childValues.map((child) => child.canonicalName)) !==
              stringify(reserved.children)
          ) {
            fail(context, (TEXT_CONTAINS_A + "forged reserved query-view schema"));
          }
          result = { canonicalName: node.value.name, coreView: node.value.name };
        } else if (node.value.name === "StatePage") {
          const [items, next] = childValues;
          const pair = items?.elementChildren;
          if (stringify(node.value.fields) !== stringify(["items", "next"]) || items?.kind !== "List" || pair?.length !== 2 || !pair[0].scalarKind || pair[0].scalarKind === "Json" || next?.canonicalName !== `${TEXT_OPTION}<${TEXT_STATECURSOR}<${pair[0].canonicalName}>>`) {
            fail(context, (TEXT_CONTAINS_A + "forged StatePage schema"));
          }
          result = { canonicalName: `StatePage<${pair[0].canonicalName}, ${pair[1].canonicalName}, ${items.capacity}>` };
        } else if (node.value.name === "QueryPage") {
          const [items, nextOffset] = childValues;
          if (
            stringify(node.value.fields) !== stringify(["items", "next_offset"]) ||
            items?.kind !== "List" ||
            items.capacity !== 64 ||
            items.listElementCoreView === undefined ||
            nextOffset?.canonicalName !== (TEXT_OPTION + "<int>")
          ) {
            fail(context, (TEXT_CONTAINS_A + "forged QueryPage schema"));
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
          tupleChildren: childValues,
        };
        break;
      case (TEXT_OPTION):
        result = { canonicalName: `${TEXT_OPTION}<${childValues[0].canonicalName}>` };
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
          elementChildren: childValues[0].tupleChildren,
        };
        break;
      case (TEXT_STATECURSOR):
        result = { canonicalName: `${TEXT_STATECURSOR}<${LEAF_TYPE_NAMES.get(node.value.kind)}>` };
        break;
      case "Unit":
        result = { canonicalName: "()" };
        break;
      case "Error":
        result = { canonicalName: node.value.identity };
        break;
      case "Leaf":
        result = { canonicalName: LEAF_TYPE_NAMES.get(node.value.kind), scalarKind: node.value.kind };
        break;
      default:
        fail(context, "contains an unsupported V1 type node");
    }
    rendered.push(result);
  }
  if (rendered.length !== 1) {
    fail(`${context}.nodes`, TEXT_IS_NOT_ONE_COMPLETE_CANONICAL_PREFIX_TYPE_TREE);
  }
  return {
    nodeCount: value.nodes.length,
    maxDepth,
    wordCount,
    canonicalName: rendered[0].canonicalName,
  };
}
