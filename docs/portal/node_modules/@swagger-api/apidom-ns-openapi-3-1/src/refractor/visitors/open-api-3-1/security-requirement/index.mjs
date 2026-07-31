import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import SecurityRequirementElement from "../../../../elements/SecurityRequirement.mjs";
/**
 * @public
 */
export const BaseSecurityRequirementVisitor = OpenApi3_1Specification.visitors.document.objects.SecurityRequirement.$visitor;
/**
 * @public
 */
class SecurityRequirementVisitor extends BaseSecurityRequirementVisitor {
  constructor(options) {
    super(options);
    this.element = new SecurityRequirementElement();
  }
}
export default SecurityRequirementVisitor;