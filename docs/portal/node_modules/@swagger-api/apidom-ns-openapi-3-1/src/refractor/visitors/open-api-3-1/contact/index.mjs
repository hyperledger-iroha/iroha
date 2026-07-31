import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import ContactElement from "../../../../elements/Contact.mjs";
/**
 * @public
 */
export const BaseContactVisitor = OpenApi3_1Specification.visitors.document.objects.Contact.$visitor;
/**
 * @public
 */
class ContactVisitor extends BaseContactVisitor {
  constructor(options) {
    super(options);
    this.element = new ContactElement();
  }
}
export default ContactVisitor;