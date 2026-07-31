import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import AmqpChannelBindingElement from "../../../../../../elements/bindings/amqp/AmqpChannelBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class AmqpChannelBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new AmqpChannelBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'amqp', 'ChannelBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default AmqpChannelBindingVisitor;