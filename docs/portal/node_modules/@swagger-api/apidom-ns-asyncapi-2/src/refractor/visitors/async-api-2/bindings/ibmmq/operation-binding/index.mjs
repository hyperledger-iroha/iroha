import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import IbmmqOperationBindingElement from "../../../../../../elements/bindings/ibmmq/IbmmqOperationBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class IbmmqOperationBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new IbmmqOperationBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'ibmmq', 'OperationBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default IbmmqOperationBindingVisitor;