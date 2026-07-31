import { specificationObj as JSONSchemaDraft7Specification } from '@swagger-api/apidom-ns-json-schema-draft-7';
import { isReferenceElement } from "../../../../predicates.mjs";
/**
 * @public
 */
export const JSONSchemaItemsVisitor = JSONSchemaDraft7Specification.visitors.document.objects.JSONSchema.fixedFields.items;

/**
 * @public
 */
class ItemsVisitor extends JSONSchemaItemsVisitor {
  ObjectElement(objectElement) {
    const result = JSONSchemaItemsVisitor.prototype.ObjectElement.call(this, objectElement);
    if (isReferenceElement(this.element)) {
      this.element.setMetaProperty('referenced-element', 'schema');
    }
    return result;
  }
  ArrayElement(arrayElement) {
    const result = JSONSchemaItemsVisitor.prototype.ArrayElement.call(this, arrayElement);

    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'schema');
    });
    return result;
  }
}
export default ItemsVisitor;