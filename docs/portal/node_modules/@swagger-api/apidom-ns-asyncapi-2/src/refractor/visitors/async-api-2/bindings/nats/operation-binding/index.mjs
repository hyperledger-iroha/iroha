import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import NatsOperationBindingElement from "../../../../../../elements/bindings/nats/NatsOperationBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class NatsOperationBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new NatsOperationBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'nats', 'OperationBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default NatsOperationBindingVisitor;