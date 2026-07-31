"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _SpecificationVisitor = _interopRequireDefault(require("../../SpecificationVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _ServerSecurity = _interopRequireDefault(require("../../../../elements/nces/ServerSecurity.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class SecurityVisitor extends (0, _tsMixer.Mixin)(_SpecificationVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _ServerSecurity.default();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const securityRequirementElement = this.toRefractedElement(['document', 'objects', 'SecurityRequirement'], item);
      this.element.push(securityRequirementElement);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = SecurityVisitor;