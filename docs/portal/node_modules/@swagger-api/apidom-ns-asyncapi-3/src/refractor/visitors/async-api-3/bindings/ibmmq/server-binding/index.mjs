import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import IbmmqServerBindingElement from "../../../../../../elements/bindings/ibmmq/IbmmqServerBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class IbmmqServerBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new IbmmqServerBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'ibmmq', 'ServerBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default IbmmqServerBindingVisitor;