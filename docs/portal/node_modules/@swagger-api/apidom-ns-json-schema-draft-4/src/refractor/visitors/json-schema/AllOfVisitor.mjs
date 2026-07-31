import { Mixin } from 'ts-mixer';
import { ArrayElement, BREAK } from '@swagger-api/apidom-core';
import FallbackVisitor from "../FallbackVisitor.mjs";
import SpecificationVisitor from "../SpecificationVisitor.mjs";
import ParentSchemaAwareVisitor from "./ParentSchemaAwareVisitor.mjs";
import { isJSONReferenceLikeElement } from "../../predicates.mjs";
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
      const specPath = isJSONReferenceLikeElement(item) ? ['document', 'objects', 'JSONReference'] : ['document', 'objects', 'JSONSchema'];
      const element = this.toRefractedElement(specPath, item);
      this.element.push(element);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default AllOfVisitor;