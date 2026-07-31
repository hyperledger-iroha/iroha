import { Mixin } from 'ts-mixer';
import { ArrayElement, BREAK } from '@swagger-api/apidom-core';
import FallbackVisitor from "../FallbackVisitor.mjs";
import SpecificationVisitor from "../SpecificationVisitor.mjs";
import ParentSchemaAwareVisitor from "./ParentSchemaAwareVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class LinksVisitor extends Mixin(SpecificationVisitor, ParentSchemaAwareVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ArrayElement();
    this.element.classes.push('json-schema-links');
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const linkDescriptionElement = this.toRefractedElement(['document', 'objects', 'LinkDescription'], item);
      this.element.push(linkDescriptionElement);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default LinksVisitor;