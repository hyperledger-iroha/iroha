import PathItemServersElement from "../../../../elements/nces/PathItemServers.mjs";
import BaseServersVisitor from "../ServersVisitor.mjs";
/**
 * @public
 */
class ServersVisitor extends BaseServersVisitor {
  constructor(options) {
    super(options);
    this.element = new PathItemServersElement();
  }
}
export default ServersVisitor;