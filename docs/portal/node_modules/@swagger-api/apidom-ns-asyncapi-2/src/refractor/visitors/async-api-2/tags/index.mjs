import { Mixin } from 'ts-mixer';
import { BREAK } from '@swagger-api/apidom-core';
import TagsElement from "../../../../elements/Tags.mjs";
import SpecificationVisitor from "../../SpecificationVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class TagsVisitor extends Mixin(SpecificationVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new TagsElement();
  }
  ArrayElement(arrayElement) {
    arrayElement.forEach(item => {
      const tagElement = this.toRefractedElement(['document', 'objects', 'Tag'], item);
      this.element.push(tagElement);
    });
    this.copyMetaAndAttributes(arrayElement, this.element);
    return BREAK;
  }
}
export default TagsVisitor;