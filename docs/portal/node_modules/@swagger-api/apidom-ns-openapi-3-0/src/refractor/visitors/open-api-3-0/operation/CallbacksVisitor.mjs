import { Mixin } from 'ts-mixer';
import { isReferenceLikeElement } from "../../../predicates.mjs";
import { isReferenceElement } from "../../../../predicates.mjs";
import OperationCallbacksElement from "../../../../elements/nces/OperationCallbacks.mjs";
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class CallbacksVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  specPath;
  constructor(options) {
    super(options);
    this.element = new OperationCallbacksElement();
    this.specPath = element => isReferenceLikeElement(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Callback'];
  }
  ObjectElement(objectElement) {
    const result = MapVisitor.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'callback');
    });
    return result;
  }
}
export default CallbacksVisitor;