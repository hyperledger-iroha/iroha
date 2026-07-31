import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import MessageExampleElement from "../../../../elements/MessageExample.mjs";
export const BaseMessageExampleVisitor = AsyncApi2Specification.visitors.document.objects.MessageExample.$visitor;
/**
 * @public
 */
class MessageExampleVisitor extends BaseMessageExampleVisitor {
  constructor(options) {
    super(options);
    this.element = new MessageExampleElement();
  }
}
export default MessageExampleVisitor;