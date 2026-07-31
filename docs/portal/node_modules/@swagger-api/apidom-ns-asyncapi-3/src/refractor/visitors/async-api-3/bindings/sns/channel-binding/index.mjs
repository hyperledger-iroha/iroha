import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import SnsChannelBindingElement from "../../../../../../elements/bindings/sns/SnsChannelBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SnsChannelBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new SnsChannelBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'sns', 'ChannelBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default SnsChannelBindingVisitor;