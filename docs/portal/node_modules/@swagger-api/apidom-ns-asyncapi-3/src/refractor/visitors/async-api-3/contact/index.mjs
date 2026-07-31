import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import ContactElement from "../../../../elements/Contact.mjs";
/**
 * @public
 */
export const BaseContactVisitor = AsyncApi2Specification.visitors.document.objects.Contact.$visitor;
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