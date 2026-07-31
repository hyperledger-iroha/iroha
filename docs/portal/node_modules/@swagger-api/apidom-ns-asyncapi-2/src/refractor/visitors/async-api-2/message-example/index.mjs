import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import MessageExampleElement from "../../../../elements/MessageExample.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MessageExampleVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new MessageExampleElement();
    this.specPath = always(['document', 'objects', 'MessageExample']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default MessageExampleVisitor;