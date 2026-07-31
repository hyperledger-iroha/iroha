import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import SolaceOperationBindingElement from "../../../../../../elements/bindings/solace/SolaceOperationBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SolaceOperationBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new SolaceOperationBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'solace', 'OperationBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default SolaceOperationBindingVisitor;