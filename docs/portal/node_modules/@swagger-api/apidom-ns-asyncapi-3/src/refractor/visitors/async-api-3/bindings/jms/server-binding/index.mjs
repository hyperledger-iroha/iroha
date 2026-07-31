import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import JmsServerBindingElement from "../../../../../../elements/bindings/jms/JmsServerBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class JmsServerBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new JmsServerBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'jms', 'ServerBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default JmsServerBindingVisitor;