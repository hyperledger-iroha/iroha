import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import PulsarMessageBindingElement from "../../../../../../elements/bindings/pulsar/PulsarMessageBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class PulsarMessageBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new PulsarMessageBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'pulsar', 'MessageBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default PulsarMessageBindingVisitor;