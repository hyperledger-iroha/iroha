import { specificationObj as AsyncApi2Specification } from '@swagger-api/apidom-ns-asyncapi-2';
import SchemaElement from "../../../../elements/Schema.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
export const BaseSchemaVisitor = AsyncApi2Specification.visitors.document.objects.Schema.$visitor;
/**
 * @public
 */
class SchemaVisitor extends BaseSchemaVisitor {
  constructor(options) {
    super(options);
    this.element = new SchemaElement();
  }
  ObjectElement(objectElement) {
    this.element = new SchemaElement();
    return FixedFieldsVisitor.prototype.ObjectElement.call(this, objectElement);
  }
}
export default SchemaVisitor;