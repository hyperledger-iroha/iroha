"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _SpecificationVisitor = _interopRequireDefault(require("../../SpecificationVisitor.cjs"));
var _StandardIdentifier = _interopRequireDefault(require("../../../../elements/StandardIdentifier.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class StandardIdentifierVisitor extends (0, _tsMixer.Mixin)(_SpecificationVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _StandardIdentifier.default();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const specPath = ['document', 'objects', 'StandardIdentifier'];
      const element = this.toRefractedElement(specPath, item);
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = StandardIdentifierVisitor;