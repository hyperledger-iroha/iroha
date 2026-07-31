import { specificationObj as JSONSchemaDraft7Specification } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { isReferenceElement } from "../../../../predicates.mjs";
/**
 * @public
 */
export const JSONSchemaDefinitionsVisitor = JSONSchemaDraft7Specification.visitors.document.objects.JSONSchema.fixedFields.definitions;

/**
 * @public
 */
class DefinitionsVisitor extends JSONSchemaDefinitionsVisitor {
  ObjectElement(objectElement) {
    const result = JSONSchemaDefinitionsVisitor.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'schema');
    });
    return result;
  }
}
export default DefinitionsVisitor;