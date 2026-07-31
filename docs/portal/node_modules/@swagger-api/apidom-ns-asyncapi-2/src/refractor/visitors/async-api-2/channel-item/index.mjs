import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import { isStringElement } from '@swagger-api/apidom-core';
import ChannelItemElement from "../../../../elements/ChannelItem.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ChannelItemVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ChannelItemElement();
    this.specPath = always(['document', 'objects', 'ChannelItem']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    const result = FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);

    // mark this ChannelItemElement with reference metadata
    if (isStringElement(this.element.$ref)) {
      this.element.classes.push('reference-element');
      this.element.setMetaProperty('referenced-element', 'channelItem');
    }
    return result;
  }
}
export default ChannelItemVisitor;