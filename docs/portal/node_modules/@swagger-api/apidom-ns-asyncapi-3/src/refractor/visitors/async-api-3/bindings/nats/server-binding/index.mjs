import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import NatsServerBindingElement from "../../../../../../elements/bindings/nats/NatsServerBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class NatsServerBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new NatsServerBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'nats', 'ServerBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default NatsServerBindingVisitor;