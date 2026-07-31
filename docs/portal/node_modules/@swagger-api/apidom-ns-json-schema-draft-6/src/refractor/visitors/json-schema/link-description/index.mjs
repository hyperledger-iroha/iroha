import { LinkDescriptionVisitor as JSONSchemaDraft4LinkDescriptionVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
import LinkDescriptionElement from "../../../../elements/LinkDescription.mjs";
/**
 * @public
 */
class LinkDescriptionVisitor extends JSONSchemaDraft4LinkDescriptionVisitor {
  constructor(options) {
    super(options);
    this.element = new LinkDescriptionElement();
  }
}
export default LinkDescriptionVisitor;