import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import MessageTraitElement from "../../../../elements/MessageTrait.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MessageTraitVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new MessageTraitElement();
    this.specPath = always(['document', 'objects', 'MessageTrait']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default MessageTraitVisitor;