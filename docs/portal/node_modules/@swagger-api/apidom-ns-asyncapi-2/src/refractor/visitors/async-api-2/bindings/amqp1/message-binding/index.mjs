import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import Amqp1MessageBindingElement from "../../../../../../elements/bindings/amqp1/Amqp1MessageBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class Amqp1MessageBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new Amqp1MessageBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'amqp1', 'MessageBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default Amqp1MessageBindingVisitor;