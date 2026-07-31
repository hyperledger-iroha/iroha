"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _jsonPointer = require("@swaggerexpert/json-pointer");
var _apidom = require("@swaggerexpert/json-pointer/evaluate/realms/apidom");
var _apidomCore = require("@swagger-api/apidom-core");
var _EvaluationJsonPointerError = _interopRequireDefault(require("./errors/EvaluationJsonPointerError.cjs"));
/**
 * Evaluates JSON Pointer against ApiDOM fragment.
 * @public
 * @deprecated
 */
const evaluate = (pointer, element) => {
  try {
    return (0, _apidom.evaluate)(element, _jsonPointer.URIFragmentIdentifier.from(pointer));
  } catch (error) {
    if (error instanceof _jsonPointer.JSONPointerEvaluateError) {
      throw new _EvaluationJsonPointerError.default(error.message, {
        pointer,
        tokens: error.referenceTokens,
        failedToken: error.referenceToken,
        failedTokenPosition: error.referenceTokenPosition,
        element: (0, _apidomCore.cloneDeep)(element),
        cause: error
      });
    }
    throw new _EvaluationJsonPointerError.default(error.message, {
      pointer,
      element: (0, _apidomCore.cloneDeep)(element),
      cause: error
    });
  }
};
var _default = exports.default = evaluate;