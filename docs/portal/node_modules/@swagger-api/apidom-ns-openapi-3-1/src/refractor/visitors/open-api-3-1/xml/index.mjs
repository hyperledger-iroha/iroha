import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import XmlElement from "../../../../elements/Xml.mjs";
/**
 * @public
 */
export const BaseXMLVisitor = OpenApi3_1Specification.visitors.document.objects.XML.$visitor;
/**
 * @public
 */
class XmlVisitor extends BaseXMLVisitor {
  constructor(options) {
    super(options);
    this.element = new XmlElement();
  }
}
export default XmlVisitor;