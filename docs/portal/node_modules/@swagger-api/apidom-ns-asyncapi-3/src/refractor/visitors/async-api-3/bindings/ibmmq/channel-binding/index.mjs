import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import IbmmqChannelBindingElement from "../../../../../../elements/bindings/ibmmq/IbmmqChannelBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class IbmmqChannelBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new IbmmqChannelBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'ibmmq', 'ChannelBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default IbmmqChannelBindingVisitor;