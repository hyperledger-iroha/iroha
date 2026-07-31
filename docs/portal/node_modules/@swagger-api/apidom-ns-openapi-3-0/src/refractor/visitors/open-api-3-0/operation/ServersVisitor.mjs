import OperationServersElement from "../../../../elements/nces/OperationServers.mjs";
import BaseServersVisitor from "../ServersVisitor.mjs";
/**
 * @public
 */
class ServersVisitor extends BaseServersVisitor {
  constructor(options) {
    super(options);
    this.element = new OperationServersElement();
  }
}
export default ServersVisitor;