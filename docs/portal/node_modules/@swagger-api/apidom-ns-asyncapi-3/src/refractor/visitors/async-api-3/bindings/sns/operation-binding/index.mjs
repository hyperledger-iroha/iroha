import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import SnsOperationBindingElement from "../../../../../../elements/bindings/sns/SnsOperationBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SnsOperationBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new SnsOperationBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'sns', 'OperationBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default SnsOperationBindingVisitor;