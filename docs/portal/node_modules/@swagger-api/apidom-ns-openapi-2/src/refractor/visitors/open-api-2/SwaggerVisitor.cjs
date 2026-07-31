"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _SwaggerVersion = _interopRequireDefault(require("../../../elements/SwaggerVersion.cjs"));
var _SpecificationVisitor = _interopRequireDefault(require("../SpecificationVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../FallbackVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class SwaggerVisitor extends (0, _tsMixer.Mixin)(_SpecificationVisitor.default, _FallbackVisitor.default) {
  StringElement(stringElement) {
    const swaggerVersionElement = new _SwaggerVersion.default((0, _apidomCore.toValue)(stringElement));
    this.copyMetaAndAttributes(stringElement, swaggerVersionElement);
    this.element = swaggerVersionElement;
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = SwaggerVisitor;