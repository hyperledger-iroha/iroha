import { specificationObj as JSONSchemaDraft7Specification } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { isReferenceElement } from "../../../../predicates.mjs";
/**
 * @public
 */
export const JSONSchemaPatternPropertiesVisitor = JSONSchemaDraft7Specification.visitors.document.objects.JSONSchema.fixedFields.patternProperties;

/**
 * @public
 */
class PatternPropertiesVisitor extends JSONSchemaPatternPropertiesVisitor {
  ObjectElement(objectElement) {
    const result = JSONSchemaPatternPropertiesVisitor.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'schema');
    });
    return result;
  }
}
export default PatternPropertiesVisitor;