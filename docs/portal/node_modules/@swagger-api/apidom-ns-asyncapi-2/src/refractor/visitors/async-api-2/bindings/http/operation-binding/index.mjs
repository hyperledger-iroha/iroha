import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import HttpOperationBindingElement from "../../../../../../elements/bindings/http/HttpOperationBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class HttpOperationBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new HttpOperationBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'http', 'OperationBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default HttpOperationBindingVisitor;