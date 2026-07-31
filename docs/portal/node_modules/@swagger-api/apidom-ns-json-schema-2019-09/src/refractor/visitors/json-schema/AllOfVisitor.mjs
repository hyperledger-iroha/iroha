import { Mixin } from 'ts-mixer';
import { ArrayElement, BREAK } from '@swagger-api/apidom-core';
import { FallbackVisitor, SpecificationVisitor, ParentSchemaAwareVisitor } from '@swagger-api/apidom-ns-json-schema-draft-7';

/**
 * @public
 */

/**
 * @public
 */
class AllOfVisitor extends Mixin(SpecificationVisitor, ParentSchemaAwareVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ArrayElement();
    this.element.classes.push('json-schema-allOf');
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const element = this.toRefractedElement(['document', 'objects', 'JSONSchema'], item);
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default AllOfVisitor;