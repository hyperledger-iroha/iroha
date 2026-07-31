import { Mixin } from 'ts-mixer';
import { T as stubTrue } from 'ramda';
import { isReferenceElement } from '@swagger-api/apidom-ns-asyncapi-2';
import AlternatingVisitor from "../../generics/AlternatingVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import { isReferenceLikeElement } from "../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ReplyVisitor extends Mixin(AlternatingVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.alternator = [{
      predicate: isReferenceLikeElement,
      specPath: ['document', 'objects', 'Reference']
    }, {
      predicate: stubTrue,
      specPath: ['document', 'objects', 'OperationReply']
    }];
  }
  ObjectElement(objectElement) {
    const result = AlternatingVisitor.prototype.enter.call(this, objectElement);
    if (isReferenceElement(this.element)) {
      this.element.setMetaProperty('referenced-element', 'operationReply');
    }
    return result;
  }
}
export default ReplyVisitor;