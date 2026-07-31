import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import Amqp1ChannelBindingElement from "../../../../../../elements/bindings/amqp1/Amqp1ChannelBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class Amqp1ChannelBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new Amqp1ChannelBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'amqp1', 'ChannelBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default Amqp1ChannelBindingVisitor;