import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import ServerVariableElement from "../../../../elements/ServerVariable.mjs";
/**
 * @public
 */
export const BaseServerVariableVisitor = AsyncApi2Specification.visitors.document.objects.ServerVariable.$visitor;
/**
 * @public
 */
class ServerVariableVisitor extends BaseServerVariableVisitor {
  constructor(options) {
    super(options);
    this.element = new ServerVariableElement();
  }
}
export default ServerVariableVisitor;