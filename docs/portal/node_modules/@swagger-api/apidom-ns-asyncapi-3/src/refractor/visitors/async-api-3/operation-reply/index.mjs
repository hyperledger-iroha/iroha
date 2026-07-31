// filepath: packages/apidom-ns-asyncapi-3/src/refractor/visitors/operation/OperationReplyVisitor.ts
import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import OperationReplyElement from "../../../../elements/OperationReply.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
class OperationReplyVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new OperationReplyElement();
    this.specPath = always(['document', 'objects', 'OperationReply']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    const result = FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);
    return result;
  }
}
export default OperationReplyVisitor;