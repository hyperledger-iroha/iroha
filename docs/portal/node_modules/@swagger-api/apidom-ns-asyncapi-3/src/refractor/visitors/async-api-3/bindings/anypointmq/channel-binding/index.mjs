import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import AnypointmqChannelBindingElement from "../../../../../../elements/bindings/anypointmq/AnypointmqChannelBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class AnypointmqChannelBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new AnypointmqChannelBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'anypointmq', 'ChannelBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default AnypointmqChannelBindingVisitor;