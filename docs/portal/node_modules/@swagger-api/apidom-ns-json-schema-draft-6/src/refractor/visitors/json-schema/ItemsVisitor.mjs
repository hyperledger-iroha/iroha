import { BREAK } from '@swagger-api/apidom-core';
import { ItemsVisitor as JSONSchemaDraft4ItemsVisitor } from '@swagger-api/apidom-ns-json-schema-draft-4';
/**
 * @public
 */
class ItemsVisitor extends JSONSchemaDraft4ItemsVisitor {
  BooleanElement(booleanElement) {
    this.element = this.toRefractedElement(['document', 'objects', 'JSONSchema'], booleanElement);
    return BREAK;
  }
}
export default ItemsVisitor;