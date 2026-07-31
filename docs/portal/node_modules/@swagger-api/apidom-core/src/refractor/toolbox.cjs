"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
exports.__esModule = true;
exports.default = void 0;
var basePredicates = _interopRequireWildcard(require("../predicates/index.cjs"));
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
  const predicates = {
    ...basePredicates
  };
  return {
    predicates,
    namespace: _namespace.default
  };
};
var _default = exports.default = createToolbox;