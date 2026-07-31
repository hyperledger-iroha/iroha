"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _jsonPointer = require("@swaggerexpert/json-pointer");
var _CompilationJsonPointerError = _interopRequireDefault(require("./errors/CompilationJsonPointerError.cjs"));
/**
 * @public
 * @deprecated
 */
const compile = tokens => {
  try {
    return _jsonPointer.URIFragmentIdentifier.to((0, _jsonPointer.compile)(tokens)).slice(1);
  } catch (error) {
    if (error instanceof _jsonPointer.JSONPointerCompileError) {
      throw new _CompilationJsonPointerError.default(error.message, {
        tokens,
        cause: error
      });
    }
    throw error;
  }
};
var _default = exports.default = compile;