import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import AnypointmqServerBindingElement from "../../../../../../elements/bindings/anypointmq/AnypointmqServerBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class AnypointmqServerBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new AnypointmqServerBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'anypointmq', 'ServerBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default AnypointmqServerBindingVisitor;