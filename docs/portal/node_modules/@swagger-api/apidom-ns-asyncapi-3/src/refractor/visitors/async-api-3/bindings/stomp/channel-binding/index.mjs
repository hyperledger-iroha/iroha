import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import StompChannelBindingElement from "../../../../../../elements/bindings/stomp/StompChannelBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class StompChannelBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new StompChannelBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'stomp', 'ChannelBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default StompChannelBindingVisitor;