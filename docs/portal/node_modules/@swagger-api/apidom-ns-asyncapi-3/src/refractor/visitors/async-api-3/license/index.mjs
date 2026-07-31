import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import LicenseElement from "../../../../elements/License.mjs";
/**
 * @public
 */
export const BaseLicenseVisitor = AsyncApi2Specification.visitors.document.objects.License.$visitor;
/**
 * @public
 */
class LicenseVisitor extends BaseLicenseVisitor {
  constructor(options) {
    super(options);
    this.element = new LicenseElement();
  }
}
export default LicenseVisitor;