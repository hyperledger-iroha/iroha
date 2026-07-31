"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _SpecificationVisitor = _interopRequireDefault(require("../../SpecificationVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _OperationMessages = _interopRequireDefault(require("../../../../elements/nces/OperationMessages.cjs"));
var _predicates = require("../../../../predicates.cjs");
/**
 * @public
 */

/**
 * Handles the 'messages' field in Operation Object (array of references)
 */
class MessagesVisitor extends (0, _tsMixer.Mixin)(_SpecificationVisitor.default, _FallbackVisitor.default) {
  constructor(options) {
    super(options);
    this.element = new _OperationMessages.default();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const specPath = ['document', 'objects', 'Reference'];
      const element = this.toRefractedElement(specPath, item);
      if ((0, _predicates.isReferenceElement)(element)) {
        element.setMetaProperty('referenced-element', 'operation-message');
      }
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = MessagesVisitor;