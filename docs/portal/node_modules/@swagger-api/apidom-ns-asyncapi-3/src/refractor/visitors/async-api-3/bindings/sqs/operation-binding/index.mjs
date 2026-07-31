import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import SqsOperationBindingElement from "../../../../../../elements/bindings/sqs/SqsOperationBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SqsOperationBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new SqsOperationBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'sqs', 'OperationBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default SqsOperationBindingVisitor;