import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import KafkaMessageBindingElement from "../../../../../../elements/bindings/kafka/KafkaMessageBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class KafkaMessageBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new KafkaMessageBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'kafka', 'MessageBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default KafkaMessageBindingVisitor;