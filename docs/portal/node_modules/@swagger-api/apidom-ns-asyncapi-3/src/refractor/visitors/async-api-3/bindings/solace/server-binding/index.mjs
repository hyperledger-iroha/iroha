import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import SolaceServerBindingElement from "../../../../../../elements/bindings/solace/SolaceServerBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SolaceServerBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new SolaceServerBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'solace', 'ServerBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default SolaceServerBindingVisitor;