"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var openApi3_0Predicates = _interopRequireWildcard(require("../predicates.cjs"));
var _namespace = _interopRequireDefault(require("../namespace.cjs"));
/**
 * @public
 */

/**
 * @public
 */

/**
 * @public
 */
const createToolbox = () => {
  const namespace = (0, _apidomCore.createNamespace)(_namespace.default);
  const predicates = {
    ...openApi3_0Predicates,
    isElement: _apidomCore.isElement,
    isStringElement: _apidomCore.isStringElement,
    isArrayElement: _apidomCore.isArrayElement,
    isObjectElement: _apidomCore.isObjectElement,
    isMemberElement: _apidomCore.isMemberElement,
    includesClasses: _apidomCore.includesClasses,
    hasElementSourceMap: _apidomCore.hasElementSourceMap
  };
  return {
    predicates,
    namespace
  };
};
var _default = exports.default = createToolbox;