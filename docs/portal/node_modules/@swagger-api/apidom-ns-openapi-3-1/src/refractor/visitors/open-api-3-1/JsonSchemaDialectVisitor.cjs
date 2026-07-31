"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _JsonSchemaDialect = _interopRequireDefault(require("../../../elements/JsonSchemaDialect.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class JsonSchemaDialectVisitor extends (0, _tsMixer.Mixin)(_apidomNsOpenapi.SpecificationVisitor, _apidomNsOpenapi.FallbackVisitor) {
  StringElement(stringElement) {
    const jsonSchemaDialectElement = new _JsonSchemaDialect.default((0, _apidomCore.toValue)(stringElement));
    this.copyMetaAndAttributes(stringElement, jsonSchemaDialectElement);
    this.element = jsonSchemaDialectElement;
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = JsonSchemaDialectVisitor;