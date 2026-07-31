import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import { FixedFieldsVisitor, FallbackVisitor } from '@swagger-api/apidom-ns-openapi-3-0';
import OpenApi3_1Element from "../../../elements/OpenApi3-1.mjs";
/**
 * @public
 */
// eslint-disable-next-line @typescript-eslint/naming-convention
/**
 * @public
 */
// eslint-disable-next-line @typescript-eslint/naming-convention
class OpenApi3_1Visitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new OpenApi3_1Element();
    this.specPath = always(['document', 'objects', 'OpenApi']);
    this.canSupportSpecificationExtensions = true;
    this.openApiSemanticElement = this.element;
  }
  ObjectElement(objectElement) {
    this.openApiGenericElement = objectElement;
    return FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);
  }
}
export default OpenApi3_1Visitor;