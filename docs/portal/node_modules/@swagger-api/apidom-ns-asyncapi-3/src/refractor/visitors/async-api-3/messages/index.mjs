import { Mixin } from 'ts-mixer';
import { isReferenceLikeElement } from '@swagger-api/apidom-ns-asyncapi-2';
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import MessagesElement from "../../../../elements/Messages.mjs";
import { isReferenceElement } from "../../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MessagesVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new MessagesElement();
    this.element.classes.push('messages');
    this.specPath = element => {
      return isReferenceLikeElement(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Message'];
    };
    this.canSupportSpecificationExtensions = false;
  }
  ObjectElement(objectElement) {
    const result = MapVisitor.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'message');
    });
    return result;
  }
}
export default MessagesVisitor;