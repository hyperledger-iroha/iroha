import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import AmqpOperationBindingElement from "../../../../../../elements/bindings/amqp/AmqpOperationBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class AmqpOperationBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new AmqpOperationBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'amqp', 'OperationBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default AmqpOperationBindingVisitor;