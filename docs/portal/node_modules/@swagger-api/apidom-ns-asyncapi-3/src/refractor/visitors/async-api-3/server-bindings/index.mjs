import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import ServerBindingsElement from "../../../../elements/ServerBindings.mjs";
export const BaseServerBindingsVisitor = AsyncApi2Specification.visitors.document.objects.ServerBindings.$visitor;
/**
 * @public
 */
class ServerBindingsVisitor extends BaseServerBindingsVisitor {
  constructor(options) {
    super(options);
    this.element = new ServerBindingsElement();
  }
}
export default ServerBindingsVisitor;