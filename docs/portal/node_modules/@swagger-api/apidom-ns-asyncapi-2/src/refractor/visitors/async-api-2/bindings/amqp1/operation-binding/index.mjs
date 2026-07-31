import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import Amqp1OperationBindingElement from "../../../../../../elements/bindings/amqp1/Amqp1OperationBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class Amqp1OperationBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new Amqp1OperationBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'amqp1', 'OperationBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default Amqp1OperationBindingVisitor;