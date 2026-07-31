import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import StompServerBindingElement from "../../../../../../elements/bindings/stomp/StompServerBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class StompServerBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new StompServerBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'stomp', 'ServerBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default StompServerBindingVisitor;