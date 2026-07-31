import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import SqsMessageBindingElement from "../../../../../../elements/bindings/sqs/SqsMessageBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SqsMessageBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new SqsMessageBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'sqs', 'MessageBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default SqsMessageBindingVisitor;