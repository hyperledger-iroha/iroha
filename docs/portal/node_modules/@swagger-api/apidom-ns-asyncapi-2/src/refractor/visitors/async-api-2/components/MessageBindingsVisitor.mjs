import { Mixin } from 'ts-mixer';
import ComponentsMessageBindingsElement from "../../../../elements/nces/ComponentsMessageBindings.mjs";
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import { isReferenceLikeElement } from "../../../predicates.mjs";
import { isReferenceElement } from "../../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MessageBindingsVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ComponentsMessageBindingsElement();
    this.specPath = element => isReferenceLikeElement(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'MessageBindings'];
  }
  ObjectElement(objectElement) {
    const result = MapVisitor.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'messageBindings');
    });
    return result;
  }
}
export default MessageBindingsVisitor;