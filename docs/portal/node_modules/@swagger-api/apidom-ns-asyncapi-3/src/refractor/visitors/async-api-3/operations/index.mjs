import { Mixin } from 'ts-mixer';
import { isReferenceLikeElement } from '@swagger-api/apidom-ns-asyncapi-2';
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import OperationsElement from "../../../../elements/Operations.mjs";
import { isReferenceElement } from "../../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class OperationsVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new OperationsElement();
    this.element.classes.push('operations');
    this.specPath = element => {
      return isReferenceLikeElement(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Operation'];
    };
    this.canSupportSpecificationExtensions = false;
  }
  ObjectElement(objectElement) {
    const result = MapVisitor.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'operation');
    });
    return result;
  }
}
export default OperationsVisitor;