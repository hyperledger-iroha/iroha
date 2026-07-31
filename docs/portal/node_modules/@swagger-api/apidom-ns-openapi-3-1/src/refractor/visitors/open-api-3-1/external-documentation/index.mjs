import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import ExternalDocumentationElement from "../../../../elements/ExternalDocumentation.mjs";
/**
 * @public
 */
export const BaseExternalDocumentationVisitor = OpenApi3_1Specification.visitors.document.objects.ExternalDocumentation.$visitor;
/**
 * @public
 */
class ExternalDocumentationVisitor extends BaseExternalDocumentationVisitor {
  constructor(options) {
    super(options);
    this.element = new ExternalDocumentationElement();
  }
}
export default ExternalDocumentationVisitor;