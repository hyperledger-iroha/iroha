import { Mixin } from 'ts-mixer';
import PatternedFieldsVisitor from "../../generics/PatternedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import ParametersElement from "../../../../elements/Parameters.mjs";
import { isReferenceLikeElement } from "../../../predicates.mjs";
import { isReferenceElement } from "../../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ParametersVisitor extends Mixin(PatternedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ParametersElement();
    this.specPath = element => isReferenceLikeElement(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Parameter'];
    this.canSupportSpecificationExtensions = false;
    this.fieldPatternPredicate = value => typeof value === 'string' && /^[A-Za-z0-9_-]+$/.test(value);
  }
  ObjectElement(objectElement) {
    const result = PatternedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'parameter');
    });
    return result;
  }
}
export default ParametersVisitor;