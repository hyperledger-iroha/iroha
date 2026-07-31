import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import MessageTraitElement from "../../../../elements/MessageTrait.mjs";
export const BaseMessageTraitVisitor = AsyncApi2Specification.visitors.document.objects.MessageTrait.$visitor;
/**
 * @public
 */
class MessageTraitVisitor extends BaseMessageTraitVisitor {
  constructor(options) {
    super(options);
    this.element = new MessageTraitElement();
  }
}
export default MessageTraitVisitor;