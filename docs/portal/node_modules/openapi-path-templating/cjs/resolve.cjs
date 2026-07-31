"use strict";

exports.__esModule = true;
exports.encodePathComponent = exports.default = void 0;
var _index = _interopRequireDefault(require("./parse/index.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const isEncoded = parameterValue => {
  try {
    return typeof parameterValue === 'string' && decodeURIComponent(parameterValue) !== parameterValue;
  } catch {
    return false;
  }
};
const encodePathComponent = parameterValue => {
  if (isEncoded(parameterValue)) {
    return parameterValue;
  }
  return encodeURIComponent(parameterValue).replace(/%5B/g, '[').replace(/%5D/g, ']');
};
exports.encodePathComponent = encodePathComponent;
const significantTypes = ['slash', 'path-literal', 'template-expression-param-name'];
const resolve = (pathTemplate, parameters, options = {}) => {
  const defaultOptions = {
    encoder: encodePathComponent
  };
  const mergedOptions = {
    ...defaultOptions,
    ...options
  };
  const parseResult = (0, _index.default)(pathTemplate);
  if (!parseResult.result.success) return pathTemplate;
  const parts = [];
  parseResult.ast.translate(parts);
  const resolvedParts = parts.filter(([type]) => significantTypes.includes(type)).map(([type, value]) => {
    if (type === 'template-expression-param-name') {
      return Object.prototype.hasOwnProperty.call(parameters, value) ? mergedOptions.encoder(parameters[value], value) : `{${value}}`;
    }
    return value;
  });
  return resolvedParts.join('');
};
var _default = exports.default = resolve;