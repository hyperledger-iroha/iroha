"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _predicates = require("../../../predicates.cjs");
var _predicates2 = require("../../../../predicates.cjs");
var _PathItemParameters = _interopRequireDefault(require("../../../../elements/nces/PathItemParameters.cjs"));
var _SpecificationVisitor = _interopRequireDefault(require("../../SpecificationVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class ParametersVisitor extends (0, _tsMixer.Mixin)(_SpecificationVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _PathItemParameters.default();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const specPath = (0, _predicates.isReferenceLikeElement)(item) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Parameter'];
      const element = this.toRefractedElement(specPath, item);
      if ((0, _predicates2.isReferenceElement)(element)) {
        element.setMetaProperty('referenced-element', 'parameter');
      }
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = ParametersVisitor;