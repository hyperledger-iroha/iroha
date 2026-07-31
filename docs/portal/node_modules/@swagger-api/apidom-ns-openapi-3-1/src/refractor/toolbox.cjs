"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
exports.__esModule = true;
exports.default = exports.ancestorLineageToJSONPointer = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _modern = require("@swagger-api/apidom-json-pointer/modern");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var openApi3_1Predicates = _interopRequireWildcard(require("../predicates.cjs"));
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

/**
 * Translates visitor ancestor lineage to a JSON Pointer tokens.
 * Ancestor lineage is constructed of following visitor method arguments:
 *
 *  - ancestors
 *  - parent
 *  - element
 * @public
 */
const ancestorLineageToJSONPointer = elementPath => {
  const jsonPointerTokens = elementPath.reduce((path, element, index) => {
    if ((0, _apidomCore.isMemberElement)(element)) {
      const token = String((0, _apidomCore.toValue)(element.key));
      path.push(token);
    } else if ((0, _apidomCore.isArrayElement)(elementPath[index - 2])) {
      const token = String(elementPath[index - 2].content.indexOf(element));
      path.push(token);
    }
    return path;
  }, []);
  return (0, _modern.compile)(jsonPointerTokens);
};

/**
 * @public
 */
exports.ancestorLineageToJSONPointer = ancestorLineageToJSONPointer;
const createToolbox = () => {
  const namespace = (0, _apidomCore.createNamespace)(_namespace.default);
  const predicates = {
    ...openApi3_1Predicates,
    isElement: _apidomCore.isElement,
    isStringElement: _apidomCore.isStringElement,
    isArrayElement: _apidomCore.isArrayElement,
    isObjectElement: _apidomCore.isObjectElement,
    isMemberElement: _apidomCore.isMemberElement,
    isServersElement: _apidomNsOpenapi.isServersElement,
    includesClasses: _apidomCore.includesClasses,
    hasElementSourceMap: _apidomCore.hasElementSourceMap
  };
  return {
    predicates,
    ancestorLineageToJSONPointer,
    namespace
  };
};
var _default = exports.default = createToolbox;