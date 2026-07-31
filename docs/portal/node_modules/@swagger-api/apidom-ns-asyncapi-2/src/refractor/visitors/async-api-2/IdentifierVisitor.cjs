"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _FallbackVisitor = _interopRequireDefault(require("../FallbackVisitor.cjs"));
var _SpecificationVisitor = _interopRequireDefault(require("../SpecificationVisitor.cjs"));
var _Identifier = _interopRequireDefault(require("../../../elements/Identifier.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class IdentifierVisitor extends (0, _tsMixer.Mixin)(_SpecificationVisitor.default, _FallbackVisitor.default) {
  StringElement(stringElement) {
    const identifierElement = new _Identifier.default((0, _apidomCore.toValue)(stringElement));
    this.copyMetaAndAttributes(stringElement, identifierElement);
    this.element = identifierElement;
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = IdentifierVisitor;