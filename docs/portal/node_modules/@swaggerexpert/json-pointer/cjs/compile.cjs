"use strict";

exports.__esModule = true;
exports.default = void 0;
var _escape = _interopRequireDefault(require("./escape.cjs"));
var _JSONPointerCompileError = _interopRequireDefault(require("./errors/JSONPointerCompileError.cjs"));
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const compile = referenceTokens => {
  if (!Array.isArray(referenceTokens)) {
    throw new TypeError('Reference tokens must be a list of strings or numbers');
  }
  try {
    if (referenceTokens.length === 0) {
      return '';
    }
    return `/${referenceTokens.map(referenceToken => {
      if (typeof referenceToken !== 'string' && typeof referenceToken !== 'number') {
        throw new TypeError('Reference token must be a string or number');
      }
      return (0, _escape.default)(String(referenceToken));
    }).join('/')}`;
  } catch (error) {
    throw new _JSONPointerCompileError.default('Unexpected error during JSON Pointer compilation', {
      cause: error,
      referenceTokens
    });
  }
};
var _default = exports.default = compile;