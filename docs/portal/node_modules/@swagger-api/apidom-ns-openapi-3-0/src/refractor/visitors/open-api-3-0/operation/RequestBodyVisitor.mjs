import { T as stubTrue } from 'ramda';
import { isReferenceElement } from "../../../../predicates.mjs";
import { isReferenceLikeElement } from "../../../predicates.mjs";
import AlternatingVisitor from "../../generics/AlternatingVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class RequestBodyVisitor extends AlternatingVisitor {
  constructor(options) {
    super(options);
    this.alternator = [{
      predicate: isReferenceLikeElement,
      specPath: ['document', 'objects', 'Reference']
    }, {
      predicate: stubTrue,
      specPath: ['document', 'objects', 'RequestBody']
    }];
  }
  ObjectElement(objectElement) {
    const result = AlternatingVisitor.prototype.enter.call(this, objectElement);
    if (isReferenceElement(this.element)) {
      this.element.setMetaProperty('referenced-element', 'requestBody');
    }
    return result;
  }
}
export default RequestBodyVisitor;