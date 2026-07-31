"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _Visitor = _interopRequireDefault(require("./Visitor.cjs"));
/**
 * This visitor is responsible for falling back to current traversed element.
 * Given ArazzoSpecificationVisitor expects ObjectElement to be traversed. If
 * different Element is provided FallBackVisitor is responsible to assigning
 * this Element as current element.
 */

/**
 * @public
 */
class FallbackVisitor extends _Visitor.default {
  enter(element) {
    this.element = (0, _apidomCore.cloneDeep)(element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = FallbackVisitor;