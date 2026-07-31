import { T as stubTrue } from 'ramda';
import { isReferenceLikeElement } from '@swagger-api/apidom-ns-asyncapi-2';
import AlternatingVisitor from "../../generics/AlternatingVisitor.mjs";
import { isReferenceElement } from "../../../../predicates.mjs";
/**
 * @public
 */
class ExternalDocumentationOrReferenceVisitor extends AlternatingVisitor {
  constructor(options) {
    super(options);
    this.alternator = [{
      predicate: isReferenceLikeElement,
      specPath: ['document', 'objects', 'Reference']
    }, {
      predicate: stubTrue,
      specPath: ['document', 'objects', 'ExternalDocumentation']
    }];
  }
  enter(element) {
    const result = AlternatingVisitor.prototype.enter.call(this, element);
    if (isReferenceElement(this.element)) {
      this.element.setMetaProperty('referenced-element', 'externalDocumentation');
    }
    return result;
  }
}
export default ExternalDocumentationOrReferenceVisitor;