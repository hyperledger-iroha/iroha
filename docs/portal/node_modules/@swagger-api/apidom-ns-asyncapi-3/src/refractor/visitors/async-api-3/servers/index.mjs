import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import ServersElement from "../../../../elements/Servers.mjs";
/**
 * @public
 */
export const BaseServersVisitor = AsyncApi2Specification.visitors.document.objects.Servers.$visitor;
/**
 * @public
 */
class ServersVisitor extends BaseServersVisitor {
  constructor(options) {
    super(options);
    this.element = new ServersElement();
    this.element.classes.push('servers');
  }
}
export default ServersVisitor;