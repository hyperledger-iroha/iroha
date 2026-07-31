import { Mixin } from 'ts-mixer';
import { ArrayElement, BREAK } from '@swagger-api/apidom-core';
import { SpecificationVisitor, FallbackVisitor, ParentSchemaAwareVisitor } from '@swagger-api/apidom-ns-json-schema-2019-09';

/**
 * @public
 */

/**
 * @public
 */
class PrefixItemsVisitor extends Mixin(SpecificationVisitor, ParentSchemaAwareVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ArrayElement();
    this.element.classes.push('json-schema-prefixItems');
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
export default PrefixItemsVisitor;