"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _FallbackVisitor = _interopRequireDefault(require("../FallbackVisitor.cjs"));
/**
 * @public
 */
class RequiredVisitor extends _FallbackVisitor.default {
  ArrayElement(arrayElement) {
    const result = this.enter(arrayElement);
    this.element.classes.push('json-schema-required');
    return result;
  }
}
var _default = exports.default = RequiredVisitor;