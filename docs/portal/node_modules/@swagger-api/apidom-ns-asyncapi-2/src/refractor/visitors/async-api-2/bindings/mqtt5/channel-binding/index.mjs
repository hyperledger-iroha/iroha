import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import Mqtt5ChannelBindingElement from "../../../../../../elements/bindings/mqtt5/Mqtt5ChannelBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class Mqtt5ChannelBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new Mqtt5ChannelBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'mqtt5', 'ChannelBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default Mqtt5ChannelBindingVisitor;