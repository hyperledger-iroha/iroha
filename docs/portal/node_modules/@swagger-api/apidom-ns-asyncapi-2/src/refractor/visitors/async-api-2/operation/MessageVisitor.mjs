import { Mixin } from 'ts-mixer';
import { isArrayElement, BREAK } from '@swagger-api/apidom-core';
import SpecificationVisitor from "../../SpecificationVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import { isReferenceLikeElement } from "../../../predicates.mjs";
import OperationMessageMapElement from "../../../../elements/nces/OperationMessageMap.mjs";
import OperationMessageElement from "../../../../elements/nces/OperationMessage.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MessageVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  ObjectElement(objectElement) {
    if (isReferenceLikeElement(objectElement)) {
      this.element = this.toRefractedElement(['document', 'objects', 'Reference'], objectElement);
      this.element.setMetaProperty('referenced-element', 'message');
    } else if (isArrayElement(objectElement.get('oneOf'))) {
      this.element = new OperationMessageMapElement();
      const operationMessageElement = new OperationMessageElement();
      objectElement.get('oneOf').forEach(item => {
        let element;
        if (isReferenceLikeElement(item)) {
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
    return BREAK;
  }
}
export default MessageVisitor;