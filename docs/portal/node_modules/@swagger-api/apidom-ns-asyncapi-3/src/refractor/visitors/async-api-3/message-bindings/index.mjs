import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import MessageBindingsElement from "../../../../elements/MessageBindings.mjs";
export const BaseMessageBindingsVisitor = AsyncApi2Specification.visitors.document.objects.MessageBindings.$visitor;
/**
 * @public
 */
class MessageBindingsVisitor extends BaseMessageBindingsVisitor {
  constructor(options) {
    super(options);
    this.element = new MessageBindingsElement();
  }
}
export default MessageBindingsVisitor;