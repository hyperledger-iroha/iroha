import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import MqttOperationBindingElement from "../../../../../../elements/bindings/mqtt/MqttOperationBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MqttOperationBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new MqttOperationBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'mqtt', 'OperationBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default MqttOperationBindingVisitor;