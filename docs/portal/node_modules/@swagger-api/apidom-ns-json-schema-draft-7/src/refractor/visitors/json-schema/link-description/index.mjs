import { LinkDescriptionVisitor as JSONSchemaDraft6LinkDescriptionVisitor } from '@swagger-api/apidom-ns-json-schema-draft-6';
import LinkDescriptionElement from "../../../../elements/LinkDescription.mjs";
/**
 * @public
 */
class LinkDescriptionVisitor extends JSONSchemaDraft6LinkDescriptionVisitor {
  constructor(options) {
    super(options);
    this.element = new LinkDescriptionElement();
  }
}
export default LinkDescriptionVisitor;