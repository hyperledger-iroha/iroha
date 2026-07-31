import { specificationObj as JSONSchemaDraft4Specification } from '@swagger-api/apidom-ns-json-schema-draft-4';
import { isReferenceElement } from "../../../../predicates.mjs";
/**
 * @public
 */
// eslint-disable-next-line prefer-destructuring
export const JSONSchemaOrJSONReferenceVisitor = JSONSchemaDraft4Specification.visitors.JSONSchemaOrJSONReferenceVisitor;

/**
 * @public
 */
class SchemaOrReferenceVisitor extends JSONSchemaOrJSONReferenceVisitor {
  ObjectElement(objectElement) {
    const result = JSONSchemaOrJSONReferenceVisitor.prototype.enter.call(this, objectElement);
    if (isReferenceElement(this.element)) {
      this.element.setMetaProperty('referenced-element', 'schema');
    }
    return result;
  }
}
export default SchemaOrReferenceVisitor;