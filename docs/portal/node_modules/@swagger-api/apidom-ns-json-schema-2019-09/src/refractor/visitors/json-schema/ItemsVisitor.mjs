import { Mixin } from 'ts-mixer';
import { ArrayElement, BREAK } from '@swagger-api/apidom-core';
import { SpecificationVisitor, FallbackVisitor, ParentSchemaAwareVisitor } from '@swagger-api/apidom-ns-json-schema-draft-7';

/**
 * @public
 */

/**
 * @public
 */
class ItemsVisitor extends Mixin(SpecificationVisitor, ParentSchemaAwareVisitor, FallbackVisitor) {
  ObjectElement(objectElement) {
    this.element = this.toRefractedElement(['document', 'objects', 'JSONSchema'], objectElement);
    return BREAK;
  }
  ArrayElement(arrayElement) {
    this.element = new ArrayElement();
    this.element.classes.push('json-schema-items');
    arrayElement.forEach(item => {
      const element = this.toRefractedElement(['document', 'objects', 'JSONSchema'], item);
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
  BooleanElement(booleanElement) {
    this.element = this.toRefractedElement(['document', 'objects', 'JSONSchema'], booleanElement);
    return BREAK;
  }
}
export default ItemsVisitor;