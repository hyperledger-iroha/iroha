import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import ReferenceElement from "../../../../elements/Reference.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import { isReferenceLikeElement } from "../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ChannelVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ReferenceElement();
    this.specPath = always(['document', 'objects', 'Reference']);
    this.canSupportSpecificationExtensions = false;
  }
  ObjectElement(objectElement) {
    const result = FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);
    if (isReferenceLikeElement(objectElement)) {
      this.element.classes.push('reference-element');
      this.element.setMetaProperty('referenced-element', 'channel');
    }
    return result;
  }
}
export default ChannelVisitor;