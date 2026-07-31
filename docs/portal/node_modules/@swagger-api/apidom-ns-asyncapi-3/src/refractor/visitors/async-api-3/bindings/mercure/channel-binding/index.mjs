import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import MercureChannelBindingElement from "../../../../../../elements/bindings/mercure/MercureChannelBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MercureChannelBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new MercureChannelBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'mercure', 'ChannelBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default MercureChannelBindingVisitor;