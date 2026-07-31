import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import KafkaOperationBindingElement from "../../../../../../elements/bindings/kafka/KafkaOperationBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class KafkaOperationBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new KafkaOperationBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'kafka', 'OperationBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default KafkaOperationBindingVisitor;