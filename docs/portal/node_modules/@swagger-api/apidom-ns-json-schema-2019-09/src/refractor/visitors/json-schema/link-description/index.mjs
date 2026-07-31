import { LinkDescriptionVisitor as JSONSchemaDraft7LinkDescriptionVisitor } from '@swagger-api/apidom-ns-json-schema-draft-7';
import LinkDescriptionElement from "../../../../elements/LinkDescription.mjs";
/**
 * @public
 */
class LinkDescriptionVisitor extends JSONSchemaDraft7LinkDescriptionVisitor {
  constructor(options) {
    super(options);
    this.element = new LinkDescriptionElement();
  }
}
export default LinkDescriptionVisitor;