"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _SpecificationVisitor = _interopRequireDefault(require("../../SpecificationVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _MessageTraitExamples = _interopRequireDefault(require("../../../../elements/nces/MessageTraitExamples.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class ExamplesVisitor extends (0, _tsMixer.Mixin)(_SpecificationVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _MessageTraitExamples.default();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const messageExampleElement = this.toRefractedElement(['document', 'objects', 'MessageExample'], item);
      this.element.push(messageExampleElement);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = ExamplesVisitor;