// filepath: packages/apidom-ns-asyncapi-3/src/refractor/visitors/operation/OperationReplyAddressVisitor.ts
import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import OperationReplyAddressElement from "../../../../elements/OperationReplyAddress.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
class OperationReplyAddressVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new OperationReplyAddressElement();
    this.specPath = always(['document', 'objects', 'OperationReplyAddress']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default OperationReplyAddressVisitor;