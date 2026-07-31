import { Mixin } from 'ts-mixer';
import { T as stubTrue } from 'ramda';
import AlternatingVisitor from "../../generics/AlternatingVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import { isReferenceLikeElement } from "../../../predicates.mjs";
import { isReferenceElement } from "../../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class BindingsVisitor extends Mixin(AlternatingVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.alternator = [{
      predicate: isReferenceLikeElement,
      specPath: ['document', 'objects', 'Reference']
    }, {
      predicate: stubTrue,
      specPath: ['document', 'objects', 'MessageBindings']
    }];
  }
  ObjectElement(objectElement) {
    const result = AlternatingVisitor.prototype.enter.call(this, objectElement);
    if (isReferenceElement(this.element)) {
      this.element.setMetaProperty('referenced-element', 'messageBindings');
    }
    return result;
  }
}
export default BindingsVisitor;