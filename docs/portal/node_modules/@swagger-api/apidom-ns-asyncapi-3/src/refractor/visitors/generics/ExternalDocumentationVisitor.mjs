import { Mixin } from 'ts-mixer';
import { isReferenceElement } from '@swagger-api/apidom-ns-asyncapi-2';
import { T as stubTrue } from 'ramda';
import FallbackVisitor from "../FallbackVisitor.mjs";
import AlternatingVisitor from "./AlternatingVisitor.mjs";
import { isReferenceLikeElement } from "../../predicates.mjs";
/**
 * @public
 */
class ExternalDocumentationVisitor extends Mixin(AlternatingVisitor, FallbackVisitor) {
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
  ObjectElement(objectElement) {
    const result = AlternatingVisitor.prototype.enter.call(this, objectElement);
    if (isReferenceElement(this.element)) {
      this.element.setMetaProperty('referenced-element', 'ExternalDocumentation');
    }
    return result;
  }
}
export default ExternalDocumentationVisitor;