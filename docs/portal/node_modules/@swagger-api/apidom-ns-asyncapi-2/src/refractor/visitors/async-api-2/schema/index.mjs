import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import SchemaElement from "../../../../elements/Schema.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SchemaVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.specPath = always(['document', 'objects', 'Schema']);
    this.canSupportSpecificationExtensions = true;
  }
  ObjectElement(objectElement) {
    this.element = new SchemaElement();
    return FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);
  }
  BooleanElement(booleanElement) {
    const result = super.enter(booleanElement);
    this.element.classes.push('boolean-json-schema');
    return result;
  }
}
export default SchemaVisitor;