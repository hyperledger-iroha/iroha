import { isReferenceLikeElement, specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import CallbackElement from "../../../../elements/Callback.mjs";
import { isReferenceElement } from "../../../../predicates.mjs";
/**
 * @public
 */
export const BaseCallbackVisitor = OpenApi3_1Specification.visitors.document.objects.Callback.$visitor;
/**
 * @public
 */
class CallbackVisitor extends BaseCallbackVisitor {
  constructor(options) {
    super(options);
    this.element = new CallbackElement();
    this.specPath = element => {
      // @ts-ignore
      return isReferenceLikeElement(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'PathItem'];
    };
  }
  ObjectElement(objectElement) {
    const result = BaseCallbackVisitor.prototype.ObjectElement.call(this, objectElement);

    // decorate every ReferenceElement with metadata about their referencing type
    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      // @ts-ignore
      referenceElement.setMetaProperty('referenced-element', 'pathItem');
    });
    return result;
  }
}
export default CallbackVisitor;