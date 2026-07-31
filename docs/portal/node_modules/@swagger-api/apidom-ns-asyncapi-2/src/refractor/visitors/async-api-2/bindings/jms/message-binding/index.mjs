import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import JmsMessageBindingElement from "../../../../../../elements/bindings/jms/JmsMessageBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class JmsMessageBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new JmsMessageBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'jms', 'MessageBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default JmsMessageBindingVisitor;