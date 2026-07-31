"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _FallbackVisitor = _interopRequireDefault(require("../FallbackVisitor.cjs"));
var _SpecificationVisitor = _interopRequireDefault(require("../SpecificationVisitor.cjs"));
var _ArazzoSpec = _interopRequireDefault(require("../../../elements/ArazzoSpec.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class ArazzoSpecVisitor extends (0, _tsMixer.Mixin)(_SpecificationVisitor.default, _FallbackVisitor.default) {
  StringElement(stringElement) {
    const arazzoSpecElement = new _ArazzoSpec.default((0, _apidomCore.toValue)(stringElement));
    this.copyMetaAndAttributes(stringElement, arazzoSpecElement);
    this.element = arazzoSpecElement;
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = ArazzoSpecVisitor;