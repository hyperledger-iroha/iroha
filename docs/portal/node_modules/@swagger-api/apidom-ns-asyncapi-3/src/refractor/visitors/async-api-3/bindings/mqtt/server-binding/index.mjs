import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import MqttServerBindingElement from "../../../../../../elements/bindings/mqtt/MqttServerBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MqttServerBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new MqttServerBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'mqtt', 'ServerBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default MqttServerBindingVisitor;