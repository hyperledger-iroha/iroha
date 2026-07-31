import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import HttpChannelBindingElement from "../../../../../../elements/bindings/http/HttpChannelBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class HttpChannelBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new HttpChannelBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'http', 'ChannelBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default HttpChannelBindingVisitor;