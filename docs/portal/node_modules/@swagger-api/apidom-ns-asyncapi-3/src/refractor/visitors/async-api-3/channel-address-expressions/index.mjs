import { Mixin } from 'ts-mixer';
import PatternedFieldsVisitor from "../../generics/PatternedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import MessageElement from "../../../../elements/Message.mjs";
import { isReferenceLikeElement } from "../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ChannelAddressExpressionsVisitor extends Mixin(PatternedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new MessageElement();
    this.element.classes.push('servers');
    this.specPath = element => {
      return isReferenceLikeElement(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Message'];
    };
    this.canSupportSpecificationExtensions = false;
  }
  ObjectElement(objectElement) {
    const result = PatternedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'message');
    });
    return result;
  }
}
export default ChannelAddressExpressionsVisitor;