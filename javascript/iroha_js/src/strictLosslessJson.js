const STRICT_JSON_MAX_DEPTH = 128;
const STRICT_JSON_MAX_NODES = 2_000_000;

/**
 * Parse the integer-only JSON profile emitted by typed Torii endpoints.
 *
 * Native `JSON.parse` rounds integer tokens beyond `Number.MAX_SAFE_INTEGER`
 * and silently keeps only the last occurrence of a duplicate object key. This
 * decoder preserves unsafe integer tokens as `bigint`, rejects duplicate keys,
 * and accepts no non-canonical numeric spelling or malformed Unicode scalar.
 *
 * @param {string} text raw UTF-8-decoded JSON text.
 * @param {string} context human-readable error context.
 * @returns {unknown} the losslessly decoded JSON value.
 */
export function parseStrictLosslessIntegerJson(text, context) {
  if (typeof text !== "string") {
    throw new TypeError(`${context} JSON source must be a string`);
  }
  if (typeof context !== "string" || context.length === 0) {
    throw new TypeError("strict lossless JSON context must be a non-empty string");
  }

  let index = 0;
  let nodes = 0;

  const fail = (message, ErrorType = TypeError) => {
    throw new ErrorType(`${context} contains invalid JSON at character ${index}: ${message}`);
  };
  const consumeNode = (depth) => {
    nodes += 1;
    if (nodes > STRICT_JSON_MAX_NODES) {
      fail(`value exceeds the ${STRICT_JSON_MAX_NODES}-node limit`, RangeError);
    }
    if (depth > STRICT_JSON_MAX_DEPTH) {
      fail(`value exceeds the ${STRICT_JSON_MAX_DEPTH}-level nesting limit`, RangeError);
    }
  };
  const skipWhitespace = () => {
    while (
      index < text.length
      && (
        text[index] === " "
        || text[index] === "\t"
        || text[index] === "\n"
        || text[index] === "\r"
      )
    ) {
      index += 1;
    }
  };
  const appendUnicodeCodeUnit = (result, codeUnit, escaped) => {
    if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      let low;
      if (escaped) {
        if (text.slice(index, index + 2) !== "\\u") {
          fail("high surrogate must be followed by an escaped low surrogate");
        }
        const lowHex = text.slice(index + 2, index + 6);
        if (!/^[0-9A-Fa-f]{4}$/u.test(lowHex)) {
          fail("invalid Unicode escape");
        }
        low = Number.parseInt(lowHex, 16);
        index += 6;
      } else {
        if (index >= text.length) fail("unterminated high surrogate");
        low = text.charCodeAt(index);
        index += 1;
      }
      if (low < 0xdc00 || low > 0xdfff) {
        fail("high surrogate must be followed by a low surrogate");
      }
      return result + String.fromCharCode(codeUnit, low);
    }
    if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      fail("unpaired low surrogate");
    }
    return result + String.fromCharCode(codeUnit);
  };
  const parseString = () => {
    if (text[index] !== "\"") fail("expected a string");
    index += 1;
    let result = "";
    while (index < text.length) {
      const character = text[index];
      if (character === "\"") {
        index += 1;
        return result;
      }
      if (character === "\\") {
        index += 1;
        if (index >= text.length) fail("unterminated string escape");
        const escaped = text[index];
        index += 1;
        switch (escaped) {
          case "\"":
          case "\\":
          case "/":
            result += escaped;
            break;
          case "b":
            result += "\b";
            break;
          case "f":
            result += "\f";
            break;
          case "n":
            result += "\n";
            break;
          case "r":
            result += "\r";
            break;
          case "t":
            result += "\t";
            break;
          case "u": {
            const hex = text.slice(index, index + 4);
            if (!/^[0-9A-Fa-f]{4}$/u.test(hex)) fail("invalid Unicode escape");
            const codeUnit = Number.parseInt(hex, 16);
            index += 4;
            result = appendUnicodeCodeUnit(result, codeUnit, true);
            break;
          }
          default:
            fail("invalid string escape");
        }
        continue;
      }
      const codeUnit = text.charCodeAt(index);
      if (codeUnit <= 0x1f) fail("unescaped control character in string");
      index += 1;
      result = appendUnicodeCodeUnit(result, codeUnit, false);
    }
    fail("unterminated string");
  };
  const parseInteger = () => {
    const start = index;
    if (text[index] === "-") index += 1;
    if (index >= text.length) fail("incomplete number");
    if (text[index] === "0") {
      index += 1;
      if (index < text.length && /[0-9]/u.test(text[index])) {
        fail("integer tokens must not contain leading zeroes");
      }
    } else if (/[1-9]/u.test(text[index])) {
      do {
        index += 1;
      } while (index < text.length && /[0-9]/u.test(text[index]));
    } else {
      fail("invalid integer token");
    }
    if (index < text.length && /[.eE]/u.test(text[index])) {
      fail("numeric tokens must be canonical integers");
    }
    const token = text.slice(start, index);
    if (token === "-0") {
      fail(
        "numeric tokens must be canonical integers; negative zero is forbidden because zero must be an unsigned integer token",
      );
    }
    let integer;
    try {
      integer = BigInt(token);
    } catch {
      fail("invalid integer token");
    }
    if (
      integer >= BigInt(Number.MIN_SAFE_INTEGER)
      && integer <= BigInt(Number.MAX_SAFE_INTEGER)
    ) {
      return Number(integer);
    }
    return integer;
  };
  const parseValue = (depth) => {
    consumeNode(depth);
    skipWhitespace();
    if (index >= text.length) fail("unexpected end of input");
    switch (text[index]) {
      case "{": {
        index += 1;
        const record = Object.create(null);
        const keys = new Set();
        skipWhitespace();
        if (text[index] === "}") {
          index += 1;
          return record;
        }
        while (true) {
          skipWhitespace();
          const key = parseString();
          if (keys.has(key)) fail(`duplicate object key ${JSON.stringify(key)}`);
          keys.add(key);
          skipWhitespace();
          if (text[index] !== ":") fail("expected ':' after object key");
          index += 1;
          const value = parseValue(depth + 1);
          Object.defineProperty(record, key, {
            value,
            enumerable: true,
            writable: true,
            configurable: true,
          });
          skipWhitespace();
          if (text[index] === "}") {
            index += 1;
            return record;
          }
          if (text[index] !== ",") fail("expected ',' or '}' in object");
          index += 1;
        }
      }
      case "[": {
        index += 1;
        const values = [];
        skipWhitespace();
        if (text[index] === "]") {
          index += 1;
          return values;
        }
        while (true) {
          values.push(parseValue(depth + 1));
          skipWhitespace();
          if (text[index] === "]") {
            index += 1;
            return values;
          }
          if (text[index] !== ",") fail("expected ',' or ']' in array");
          index += 1;
        }
      }
      case "\"":
        return parseString();
      case "t":
        if (text.slice(index, index + 4) !== "true") fail("invalid literal");
        index += 4;
        return true;
      case "f":
        if (text.slice(index, index + 5) !== "false") fail("invalid literal");
        index += 5;
        return false;
      case "n":
        if (text.slice(index, index + 4) !== "null") fail("invalid literal");
        index += 4;
        return null;
      default:
        if (text[index] === "-" || /[0-9]/u.test(text[index])) return parseInteger();
        fail("unexpected token");
    }
  };

  skipWhitespace();
  const parsed = parseValue(0);
  skipWhitespace();
  if (index !== text.length) fail("trailing input");
  return parsed;
}
