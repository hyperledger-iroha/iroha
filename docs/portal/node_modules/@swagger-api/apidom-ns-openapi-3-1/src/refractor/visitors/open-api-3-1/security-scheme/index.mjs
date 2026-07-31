import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import SecuritySchemeElement from "../../../../elements/SecurityScheme.mjs";
/**
 * @public
 */
export const BaseSecuritySchemeVisitor = OpenApi3_1Specification.visitors.document.objects.SecurityScheme.$visitor;
/**
 * @public
 */
class SecuritySchemeVisitor extends BaseSecuritySchemeVisitor {
  constructor(options) {
    super(options);
    this.element = new SecuritySchemeElement();
  }
}
export default SecuritySchemeVisitor;