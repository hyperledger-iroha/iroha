// SPDX-License-Identifier: Apache-2.0

const SHA256_HEX = /^[0-9a-f]{64}$/;
const GIT_SHA1_HEX = /^[0-9a-f]{40}$/;

/**
 * Parse and validate a release OpenAPI document.
 *
 * Release publication requires actual route and schema inventories. A
 * syntactically valid skeleton with empty `paths` or `components.schemas`
 * must never be signed, versioned, or accepted by verification tooling.
 */
export function validateReleaseOpenApiDocumentBytes(
  bytes,
  {label = 'OpenAPI document'} = {},
) {
  let document;
  try {
    const text =
      typeof bytes === 'string' ? bytes : Buffer.from(bytes).toString('utf8');
    scanJsonRejectDuplicateKeys(text, label);
    document = JSON.parse(text);
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error?.message ?? error}`);
  }
  return validateReleaseOpenApiDocument(document, {label});
}

/**
 * Validate JSON syntax while rejecting duplicate member names before
 * `JSON.parse` can collapse them.
 */
export function scanJsonRejectDuplicateKeys(text, label) {
  let index = 0;

  function fail(message) {
    throw new Error(`${label} contains invalid JSON at offset ${index}: ${message}`);
  }

  function skipWhitespace() {
    while (
      index < text.length &&
      (text[index] === ' ' ||
        text[index] === '\n' ||
        text[index] === '\r' ||
        text[index] === '\t')
    ) {
      index += 1;
    }
  }

  function parseString() {
    const start = index;
    if (text[index] !== '"') {
      fail('expected a string');
    }
    index += 1;
    while (index < text.length) {
      const code = text.charCodeAt(index);
      if (code === 0x22) {
        index += 1;
        try {
          return JSON.parse(text.slice(start, index));
        } catch (error) {
          fail(error.message ?? String(error));
        }
      }
      if (code < 0x20) {
        fail('unescaped control character in string');
      }
      if (code === 0x5c) {
        index += 1;
        if (index >= text.length) {
          fail('unterminated escape sequence');
        }
        const escape = text[index];
        if (escape === 'u') {
          const unicode = text.slice(index + 1, index + 5);
          if (!/^[0-9a-fA-F]{4}$/.test(unicode)) {
            fail('invalid Unicode escape');
          }
          index += 5;
          continue;
        }
        if (!'"\\/bfnrt'.includes(escape)) {
          fail('invalid string escape');
        }
      }
      index += 1;
    }
    fail('unterminated string');
  }

  function parseNumber() {
    const remainder = text.slice(index);
    const match = remainder.match(
      /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/,
    );
    if (!match) {
      fail('invalid number');
    }
    index += match[0].length;
  }

  function parseArray() {
    index += 1;
    skipWhitespace();
    if (text[index] === ']') {
      index += 1;
      return;
    }
    while (index < text.length) {
      parseValue();
      skipWhitespace();
      if (text[index] === ']') {
        index += 1;
        return;
      }
      if (text[index] !== ',') {
        fail('expected comma or closing bracket');
      }
      index += 1;
      skipWhitespace();
    }
    fail('unterminated array');
  }

  function parseObject() {
    index += 1;
    skipWhitespace();
    const keys = new Set();
    if (text[index] === '}') {
      index += 1;
      return;
    }
    while (index < text.length) {
      const key = parseString();
      if (keys.has(key)) {
        throw new Error(`${label} contains duplicate JSON member ${JSON.stringify(key)}`);
      }
      keys.add(key);
      skipWhitespace();
      if (text[index] !== ':') {
        fail('expected colon after object member name');
      }
      index += 1;
      parseValue();
      skipWhitespace();
      if (text[index] === '}') {
        index += 1;
        return;
      }
      if (text[index] !== ',') {
        fail('expected comma or closing brace');
      }
      index += 1;
      skipWhitespace();
    }
    fail('unterminated object');
  }

  function parseValue() {
    skipWhitespace();
    const token = text[index];
    if (token === '{') {
      parseObject();
    } else if (token === '[') {
      parseArray();
    } else if (token === '"') {
      parseString();
    } else if (token === '-' || (token >= '0' && token <= '9')) {
      parseNumber();
    } else if (text.startsWith('true', index)) {
      index += 4;
    } else if (text.startsWith('false', index)) {
      index += 5;
    } else if (text.startsWith('null', index)) {
      index += 4;
    } else {
      fail('unexpected token');
    }
  }

  if (typeof text !== 'string') {
    throw new TypeError(`${label} JSON must be a string`);
  }
  skipWhitespace();
  parseValue();
  skipWhitespace();
  if (index !== text.length) {
    fail('trailing content');
  }
}

export function validateReleaseOpenApiDocument(
  document,
  {label = 'OpenAPI document'} = {},
) {
  if (!isObject(document)) {
    throw new Error(`${label} must be a JSON object`);
  }
  if (
    typeof document.openapi !== 'string' ||
    !document.openapi.startsWith('3.')
  ) {
    throw new Error(`${label} must declare an OpenAPI 3.x version`);
  }
  if (!isObject(document.info)) {
    throw new Error(`${label} is missing the info object`);
  }
  for (const field of ['title', 'version']) {
    if (
      typeof document.info[field] !== 'string' ||
      document.info[field].trim().length === 0
    ) {
      throw new Error(`${label} info.${field} must be a non-empty string`);
    }
  }
  if (!isObject(document.paths)) {
    throw new Error(`${label} is missing the paths object`);
  }
  if (Object.keys(document.paths).length === 0) {
    throw new Error(
      `${label} must define at least one path; empty/stub specifications are forbidden`,
    );
  }
  if (!isObject(document.components?.schemas)) {
    throw new Error(`${label} is missing components.schemas`);
  }
  if (Object.keys(document.components.schemas).length === 0) {
    throw new Error(
      `${label} must define at least one component schema; empty/stub specifications are forbidden`,
    );
  }
  return document;
}

/**
 * Validate the V2 OpenAPI generator provenance contract.
 *
 * Every manifest binds the canonical generator-input inventory with a
 * lowercase, nonzero SHA-256 digest. Dirty development manifests must also be
 * unsigned and cannot be release-verified.
 */
export function validateOpenApiGeneratorProvenance(
  manifest,
  {label = 'OpenAPI manifest', signed = false, requireClean = false} = {},
) {
  if (manifest?.version !== 2) {
    throw new Error(`${label} provenance requires manifest version exactly 2`);
  }
  const dirtyField = manifest?.generator_dirty;
  if (typeof dirtyField !== 'boolean') {
    throw new Error(`${label} generator_dirty must be boolean and is required`);
  }
  const dirty = dirtyField;
  const commit = manifest?.generator_commit;
  const sourceDigest = manifest?.generator_source_sha256_hex;
  if (
    typeof sourceDigest !== 'string' ||
    !SHA256_HEX.test(sourceDigest) ||
    /^0{64}$/.test(sourceDigest)
  ) {
    throw new Error(
      `${label} provenance requires generator_source_sha256_hex as 64 lowercase hexadecimal characters and it must be nonzero`,
    );
  }

  if (dirty) {
    if (commit !== null) {
      throw new Error(`${label} dirty provenance must set generator_commit to null`);
    }
    if (signed) {
      throw new Error(`${label} dirty provenance must not be signed`);
    }
    if (requireClean) {
      throw new Error(`${label} dirty provenance cannot be release-verified`);
    }
    return {dirty: true, commit: null, sourceSha256Hex: sourceDigest};
  }

  if (typeof commit !== 'string' || !GIT_SHA1_HEX.test(commit)) {
    throw new Error(
      `${label} clean provenance requires generator_commit as exactly 40 lowercase hexadecimal characters`,
    );
  }
  if (/^0{40}$/.test(commit)) {
    throw new Error(`${label} generator_commit must identify a nonzero Git commit`);
  }
  return {dirty: false, commit, sourceSha256Hex: sourceDigest};
}

function isObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
