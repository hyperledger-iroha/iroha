import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import SecuritySchemeElement from "../../../../elements/SecurityScheme.mjs";
export const BaseSecuritySchemeVisitor = AsyncApi2Specification.visitors.document.objects.SecurityScheme.$visitor;
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