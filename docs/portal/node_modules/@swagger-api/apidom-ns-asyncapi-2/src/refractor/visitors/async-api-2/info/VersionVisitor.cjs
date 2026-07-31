"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
/**
 * @public
 */
class VersionVisitor extends _FallbackVisitor.default {
  StringElement(stringElement) {
    this.element = new _apidomCore.StringElement((0, _apidomCore.toValue)(stringElement));
    this.copyMetaAndAttributes(stringElement, this.element);
    this.element.classes.push('api-version');
    this.element.classes.push('version');
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = VersionVisitor;