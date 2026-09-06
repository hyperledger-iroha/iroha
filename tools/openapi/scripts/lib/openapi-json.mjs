// SPDX-License-Identifier: Apache-2.0

/**
 * Validate JSON syntax while rejecting duplicate object member names before
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
