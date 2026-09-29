// SPDX-License-Identifier: Apache-2.0

import { KAGEMUSHA_RELEASE_SCHEMAS_V1 } from "./governanceKagemushaReleaseSchemasV1.js";

/** Validate one closed, bounded first-release KAGEMUSHA governance JSON shape. */
export function validateKagemushaReleaseSchemaV1(name, value) {
  const schema = KAGEMUSHA_RELEASE_SCHEMAS_V1[name];
  if (schema === undefined) {
    throw new TypeError(`unknown KAGEMUSHA release schema: ${name}`);
  }
  validateSchemaValue(value, schema, name, { visited: 0 }, 0);
}

function validateSchemaValue(value, schema, context, budget, depth) {
  budget.visited += 1;
  if (budget.visited > 250_000 || depth > 64) {
    throw new TypeError(`${context} exceeds the release schema traversal bound`);
  }
  if (schema.$ref !== undefined) {
    const prefix = "#/components/schemas/";
    if (typeof schema.$ref !== "string" || !schema.$ref.startsWith(prefix)) {
      throw new TypeError(`${context} has an unrecognized release schema reference`);
    }
    const target = KAGEMUSHA_RELEASE_SCHEMAS_V1[schema.$ref.slice(prefix.length)];
    if (target === undefined) {
      throw new TypeError(`${context} has an unrecognized release schema target`);
    }
    validateSchemaValue(value, target, context, budget, depth + 1);
  }
  if (schema.oneOf !== undefined) {
    let matching = 0;
    for (const choice of schema.oneOf) {
      try {
        validateSchemaValue(value, choice, context, budget, depth + 1);
        matching += 1;
      } catch (error) {
        if (!(error instanceof TypeError)) throw error;
      }
    }
    if (matching !== 1) {
      throw new TypeError(`${context} must match exactly one V1 release shape`);
    }
  }

  switch (schema.type) {
    case "object": {
      if (!plainRecord(value)) throw new TypeError(`${context} must be an object`);
      if (schema.additionalProperties !== false) {
        throw new TypeError(`${context} has an unclosed release schema`);
      }
      const properties = schema.properties ?? {};
      for (const key of schema.required ?? []) {
        if (!Object.hasOwn(value, key)) {
          throw new TypeError(`${context} is missing required field '${key}'`);
        }
      }
      for (const [key, item] of Object.entries(value)) {
        if (!Object.hasOwn(properties, key)) {
          throw new TypeError(`${context} has unknown field '${key}'`);
        }
        validateSchemaValue(item, properties[key], `${context}.${key}`, budget, depth + 1);
      }
      break;
    }
    case "array": {
      if (!Array.isArray(value)) throw new TypeError(`${context} must be an array`);
      if (value.length < (schema.minItems ?? 0) || value.length > (schema.maxItems ?? 250_000)) {
        throw new TypeError(`${context} has an invalid array length`);
      }
      for (let index = 0; index < value.length; index += 1) {
        if (!Object.hasOwn(value, index)) {
          throw new TypeError(`${context}[${index}] is missing an array item`);
        }
      }
      if (schema.uniqueItems) {
        const keys = value.map((item) => JSON.stringify(item));
        if (new Set(keys).size !== keys.length) {
          throw new TypeError(`${context} must have unique array items`);
        }
      }
      if (schema.items === undefined) {
        throw new TypeError(`${context} has an unclosed array schema`);
      }
      for (let index = 0; index < value.length; index += 1) {
        validateSchemaValue(value[index], schema.items, `${context}[${index}]`, budget, depth + 1);
      }
      break;
    }
    case "integer": {
      if (!Number.isSafeInteger(value)) throw new TypeError(`${context} must be an integer`);
      const width = { uint8: 8, uint16: 16, uint32: 32, uint64: 64 }[schema.format];
      if (width !== undefined && (value < 0 || (width < 64 && value >= 2 ** width))) {
        throw new TypeError(`${context} exceeds its unsigned integer width`);
      }
      if (value < (schema.minimum ?? value) || value > (schema.maximum ?? value)) {
        throw new TypeError(`${context} is outside its V1 integer range`);
      }
      break;
    }
    case "string": {
      if (typeof value !== "string") throw new TypeError(`${context} must be a string`);
      if (value.length < (schema.minLength ?? 0) || value.length > (schema.maxLength ?? value.length)) {
        throw new TypeError(`${context} has an invalid string length`);
      }
      if (schema.pattern !== undefined && !new RegExp(`^(?:${schema.pattern})$`, "u").test(value)) {
        throw new TypeError(`${context} is not a canonical V1 string`);
      }
      break;
    }
    case "null":
      if (value !== null) throw new TypeError(`${context} must be null`);
      break;
    case undefined:
      break;
    default:
      throw new TypeError(`${context} has an unrecognized release schema type`);
  }
  if (Object.hasOwn(schema, "const") && !sameJson(value, schema.const)) {
    throw new TypeError(`${context} has the wrong V1 constant`);
  }
  if (schema.enum !== undefined && !schema.enum.some((item) => sameJson(value, item))) {
    throw new TypeError(`${context} is outside the closed V1 enum`);
  }
  if (schema.not !== undefined) {
    let prohibited = false;
    try {
      validateSchemaValue(value, schema.not, context, budget, depth + 1);
      prohibited = true;
    } catch (error) {
      if (!(error instanceof TypeError)) throw error;
    }
    if (prohibited) throw new TypeError(`${context} has a prohibited V1 value`);
  }
}

function plainRecord(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function sameJson(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}
