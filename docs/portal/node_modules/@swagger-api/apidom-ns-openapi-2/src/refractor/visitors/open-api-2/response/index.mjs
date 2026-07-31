import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import ResponseElement from "../../../../elements/Response.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ResponseVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ResponseElement();
    this.specPath = always(['document', 'objects', 'Response']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default ResponseVisitor;