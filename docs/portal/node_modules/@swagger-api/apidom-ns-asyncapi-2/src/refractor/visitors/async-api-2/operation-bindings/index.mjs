import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import OperationBindingsElement from "../../../../elements/OperationBindings.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class OperationBindingsVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new OperationBindingsElement();
    this.specPath = always(['document', 'objects', 'OperationBindings']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default OperationBindingsVisitor;