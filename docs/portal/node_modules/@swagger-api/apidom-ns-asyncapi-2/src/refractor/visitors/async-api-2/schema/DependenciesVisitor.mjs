import { specificationObj as JSONSchemaDraft7Specification } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { isReferenceElement } from "../../../../predicates.mjs";
/**
 * @public
 */
export const JSONSchemaDependenciesVisitor = JSONSchemaDraft7Specification.visitors.document.objects.JSONSchema.fixedFields.dependencies;

/**
 * @public
 */
class DependenciesVisitor extends JSONSchemaDependenciesVisitor {
  ObjectElement(objectElement) {
    const result = JSONSchemaDependenciesVisitor.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'schema');
    });
    return result;
  }
}
export default DependenciesVisitor;