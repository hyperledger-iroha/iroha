import { BREAK, cloneDeep } from '@swagger-api/apidom-core';
import OperationTagsElement from "../../../../elements/nces/OperationTags.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
class TagsVisitor extends FallbackVisitor {
  constructor(options) {
    super(options);
    this.element = new OperationTagsElement();
  }
  ArrayElement(arrayElement) {
    this.element = this.element.concat(cloneDeep(arrayElement));
    return BREAK;
  }
}
export default TagsVisitor;