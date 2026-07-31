import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import AnypointmqOperationBindingElement from "../../../../../../elements/bindings/anypointmq/AnypointmqOperationBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class AnypointmqOperationBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new AnypointmqOperationBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'anypointmq', 'OperationBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default AnypointmqOperationBindingVisitor;