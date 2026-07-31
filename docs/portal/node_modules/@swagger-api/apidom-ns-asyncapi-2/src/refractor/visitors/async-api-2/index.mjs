import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import FixedFieldsVisitor from "../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../FallbackVisitor.mjs";
import AsyncApi2Element from "../../../elements/AsyncApi2.mjs";
/**
 * @public
 */
/**
 * @public
 */
class AsyncApi2Visitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new AsyncApi2Element();
    this.specPath = always(['document', 'objects', 'AsyncApi']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default AsyncApi2Visitor;