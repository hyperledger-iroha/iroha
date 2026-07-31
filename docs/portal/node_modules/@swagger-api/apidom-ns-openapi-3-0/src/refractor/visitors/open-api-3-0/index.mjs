import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import FixedFieldsVisitor from "../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../FallbackVisitor.mjs";
import OpenApi3_0Element from "../../../elements/OpenApi3-0.mjs";
/**
 * @public
 */
// eslint-disable-next-line @typescript-eslint/naming-convention
/**
 * @public
 */
// eslint-disable-next-line @typescript-eslint/naming-convention
class OpenApi3_0Visitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new OpenApi3_0Element();
    this.specPath = always(['document', 'objects', 'OpenApi']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    return FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);
  }
}
export default OpenApi3_0Visitor;