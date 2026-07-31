import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import ChannelElement from "../../../../elements/Channel.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
class ChannelVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ChannelElement();
    this.specPath = always(['document', 'objects', 'Channel']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    const result = FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);
    return result;
  }
}
export default ChannelVisitor;