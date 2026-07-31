import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import MessageElement from "../../../../elements/Message.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MessageVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new MessageElement();
    this.specPath = always(['document', 'objects', 'Message']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default MessageVisitor;