"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _FallbackVisitor = _interopRequireDefault(require("../FallbackVisitor.cjs"));
/**
 * @public
 */
class BasePathVisitor extends _FallbackVisitor.default {
  StringElement(stringElement) {
    const result = super.enter(stringElement);
    this.element.classes.push('swagger-base-path');
    return result;
  }
}
var _default = exports.default = BasePathVisitor;