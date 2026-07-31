import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import JmsOperationBindingElement from "../../../../../../elements/bindings/jms/JmsOperationBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class JmsOperationBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new JmsOperationBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'jms', 'OperationBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default JmsOperationBindingVisitor;