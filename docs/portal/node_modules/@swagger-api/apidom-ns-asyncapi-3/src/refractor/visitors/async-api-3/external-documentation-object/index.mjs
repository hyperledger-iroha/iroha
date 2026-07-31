import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import ExternalDocumentationElement from "../../../../elements/ExternalDocumentation.mjs";
export const BaseExternalDocumentationVisitor = AsyncApi2Specification.visitors.document.objects.ExternalDocumentation.$visitor;
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