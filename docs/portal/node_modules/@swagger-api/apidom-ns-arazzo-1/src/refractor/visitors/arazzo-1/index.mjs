import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import FixedFieldsVisitor from "../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../FallbackVisitor.mjs";
import ArazzoSpecification1Element from "../../../elements/ArazzoSpecification1.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ArazzoSpecificationVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  element;
  specPath;
  canSupportSpecificationExtensions;
  constructor(options) {
    super(options);
    this.element = new ArazzoSpecification1Element();
    this.specPath = always(['document', 'objects', 'ArazzoSpecification']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    return FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);
  }
}
export default ArazzoSpecificationVisitor;