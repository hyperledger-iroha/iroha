import { BREAK, cloneDeep } from '@swagger-api/apidom-core';
import SpecificationVisitor from "./SpecificationVisitor.mjs";
/**
 * @public
 */
class SpecificationExtensionVisitor extends SpecificationVisitor {
  MemberElement(memberElement) {
    this.element = cloneDeep(memberElement);
    this.element.classes.push('specification-extension');
    return BREAK;
  }
}
export default SpecificationExtensionVisitor;