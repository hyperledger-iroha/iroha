import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import OperationTraitElement from "../../../../elements/OperationTrait.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class OperationTraitVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new OperationTraitElement();
    this.specPath = always(['document', 'objects', 'OperationTrait']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default OperationTraitVisitor;