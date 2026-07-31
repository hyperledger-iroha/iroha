import { specificationObj as JSONSchemaDraft4Specification } from '@swagger-api/apidom-ns-json-schema-draft-4';
/**
 * @public
 */
export const JSONSchemaTypeVisitor = JSONSchemaDraft4Specification.visitors.document.objects.JSONSchema.fixedFields.type;

/**
 * @public
 */
class TypeVisitor extends JSONSchemaTypeVisitor {
  ArrayElement(arrayElement) {
    const result = this.enter(arrayElement);
    return result;
  }
}
export default TypeVisitor;