import { LinkDescriptionVisitor as JSONSchemaDraft201909LinkDescriptionVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import LinkDescriptionElement from "../../../../elements/LinkDescription.mjs";
/**
 * @public
 */
class LinkDescriptionVisitor extends JSONSchemaDraft201909LinkDescriptionVisitor {
  constructor(options) {
    super(options);
    this.element = new LinkDescriptionElement();
  }
}
export default LinkDescriptionVisitor;