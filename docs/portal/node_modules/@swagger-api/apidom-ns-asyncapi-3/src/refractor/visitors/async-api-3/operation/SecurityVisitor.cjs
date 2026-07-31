"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _SpecificationVisitor = _interopRequireDefault(require("../../SpecificationVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _OperationSecurity = _interopRequireDefault(require("../../../../elements/nces/OperationSecurity.cjs"));
var _predicates = require("../../../predicates.cjs");
/**
 * @public
 */

/**
 * @public
 */
class SecurityVisitor extends (0, _tsMixer.Mixin)(_SpecificationVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _OperationSecurity.default();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      let element;
      if ((0, _predicates.isReferenceLikeElement)(item)) {
        element = this.toRefractedElement(['document', 'objects', 'Reference'], item);
        element.setMetaProperty('referenced-element', 'securityScheme');
      } else {
        element = this.toRefractedElement(['document', 'objects', 'SecurityScheme'], item);
      }
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = SecurityVisitor;