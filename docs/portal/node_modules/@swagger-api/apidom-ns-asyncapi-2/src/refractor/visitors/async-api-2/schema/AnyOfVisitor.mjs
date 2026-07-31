import { specificationObj as JSONSchemaDraft7Specification } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { isReferenceElement } from "../../../../predicates.mjs";
/**
 * @public
 */
export const JSONSchemaAnyOfVisitor = JSONSchemaDraft7Specification.visitors.document.objects.JSONSchema.fixedFields.anyOf;

/**
 * @public
 */
class AnyOfVisitor extends JSONSchemaAnyOfVisitor {
  ArrayElement(arrayElement) {
    const result = JSONSchemaAnyOfVisitor.prototype.ArrayElement.call(this, arrayElement);

    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'schema');
    });
    return result;
  }
}
export default AnyOfVisitor;