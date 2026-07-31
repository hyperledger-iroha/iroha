import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import IbmmqMessageBindingElement from "../../../../../../elements/bindings/ibmmq/IbmmqMessageBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class IbmmqMessageBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new IbmmqMessageBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'ibmmq', 'MessageBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default IbmmqMessageBindingVisitor;