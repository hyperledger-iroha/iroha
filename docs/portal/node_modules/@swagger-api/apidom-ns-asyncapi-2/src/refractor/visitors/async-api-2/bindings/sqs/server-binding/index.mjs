import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import SqsServerBindingElement from "../../../../../../elements/bindings/sqs/SqsServerBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SqsServerBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new SqsServerBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'sqs', 'ServerBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default SqsServerBindingVisitor;