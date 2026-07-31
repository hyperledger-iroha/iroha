import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import HttpServerBindingElement from "../../../../../../elements/bindings/http/HttpServerBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class HttpServerBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new HttpServerBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'http', 'ServerBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default HttpServerBindingVisitor;