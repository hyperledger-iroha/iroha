import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import MercureMessageBindingElement from "../../../../../../elements/bindings/mercure/MercureMessageBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MercureMessageBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new MercureMessageBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'mercure', 'MessageBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default MercureMessageBindingVisitor;