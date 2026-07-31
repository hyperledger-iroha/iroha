"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _tsMixer = require("ts-mixer");
var _apidomCore = require("@swagger-api/apidom-core");
var _SpecificationVisitor = _interopRequireDefault(require("../../SpecificationVisitor.cjs"));
var _FallbackVisitor = _interopRequireDefault(require("../../FallbackVisitor.cjs"));
var _predicates = require("../../../predicates.cjs");
var _OperationMessageMap = _interopRequireDefault(require("../../../../elements/nces/OperationMessageMap.cjs"));
var _OperationMessage = _interopRequireDefault(require("../../../../elements/nces/OperationMessage.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class MessageVisitor extends (0, _tsMixer.Mixin)(_SpecificationVisitor.default, _FallbackVisitor.default) {
  ObjectElement(objectElement) {
    if ((0, _predicates.isReferenceLikeElement)(objectElement)) {
      this.element = this.toRefractedElement(['document', 'objects', 'Reference'], objectElement);
      this.element.setMetaProperty('referenced-element', 'message');
    } else if ((0, _apidomCore.isArrayElement)(objectElement.get('oneOf'))) {
      this.element = new _OperationMessageMap.default();
      const operationMessageElement = new _OperationMessage.default();
      objectElement.get('oneOf').forEach(item => {
        let element;
        if ((0, _predicates.isReferenceLikeElement)(item)) {
          element = this.toRefractedElement(['document', 'objects', 'Reference'], item);
          element.setMetaProperty('referenced-element', 'message');
        } else {
          element = this.toRefractedElement(['document', 'objects', 'Message'], item);
        }
        operationMessageElement.push(element);
      });
      this.element.oneOf = operationMessageElement;
    } else {
      this.element = this.toRefractedElement(['document', 'objects', 'Message'], objectElement);
    }
    this.copyMetaAndAttributes(objectElement, this.element);
    return _apidomCore.BREAK;
  }
}
var _default = exports.default = MessageVisitor;