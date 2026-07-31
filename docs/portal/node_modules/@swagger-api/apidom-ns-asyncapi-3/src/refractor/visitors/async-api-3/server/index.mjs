import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import ServerElement from "../../../../elements/Server.mjs";
/**
 * @public
 */
export const BaseServerVisitor = AsyncApi2Specification.visitors.document.objects.Server.$visitor;
/**
 * @public
 */
class ServerVisitor extends BaseServerVisitor {
  constructor(options) {
    super(options);
    this.element = new ServerElement();
  }
}
export default ServerVisitor;