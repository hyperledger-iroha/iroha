import { Mixin } from 'ts-mixer';
import { BREAK } from '@swagger-api/apidom-core';
import { isReferenceElement } from '@swagger-api/apidom-ns-asyncapi-2';
import OperationReplyMessagesElement from "../../../../elements/nces/OperationReplyMessages.mjs";
import SpecificationVisitor from "../../SpecificationVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MessagesVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new OperationReplyMessagesElement();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const specPath = ['document', 'objects', 'Reference'];
      const element = this.toRefractedElement(specPath, item);
      if (isReferenceElement(element)) {
        element.setMetaProperty('referenced-element', 'operation-reply-message');
      }
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default MessagesVisitor;