"use strict";

exports.__esModule = true;
exports.encodeServerVariable = exports.default = void 0;
var _index = _interopRequireDefault(require("./parse/index.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const isEncoded = serverVariableValue => {
  try {
    return typeof serverVariableValue === 'string' && decodeURIComponent(serverVariableValue) !== serverVariableValue;
  } catch {
    return false;
  }
};
const encodeServerVariable = serverVariableValue => {
  if (isEncoded(serverVariableValue)) {
    return serverVariableValue;
  }
  return encodeURIComponent(serverVariableValue).replace(/%5B/g, '[').replace(/%5D/g, ']');
};
exports.encodeServerVariable = encodeServerVariable;
const significantTypes = ['literals', 'server-variable-name'];
const substitute = (serverURLTemplate, serverVariables, options = {}) => {
  const defaultOptions = {
    encoder: encodeServerVariable
  };
  const mergedOptions = {
    ...defaultOptions,
    ...options
  };
  const parseResult = (0, _index.default)(serverURLTemplate);
  if (!parseResult.result.success) return serverURLTemplate;
  const parts = [];
  parseResult.ast.translate(parts);
  const substitutedParts = parts.filter(([type]) => significantTypes.includes(type)).map(([type, value]) => {
    if (type === 'server-variable-name') {
      return Object.hasOwn(serverVariables, value) ? mergedOptions.encoder(serverVariables[value], value) : `{${value}}`;
    }
    return value;
  });
  return substitutedParts.join('');
};
var _default = exports.default = substitute;