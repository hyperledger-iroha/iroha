import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import KafkaServerBindingElement from "../../../../../../elements/bindings/kafka/KafkaServerBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class KafkaServerBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new KafkaServerBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'kafka', 'ServerBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default KafkaServerBindingVisitor;