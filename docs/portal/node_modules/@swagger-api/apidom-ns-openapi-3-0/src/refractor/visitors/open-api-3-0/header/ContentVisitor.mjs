import BaseContentVisitor from "../ContentVisitor.mjs";
import HeaderContentElement from "../../../../elements/nces/HeaderContent.mjs";
/**
 * @public
 */
class ContentVisitor extends BaseContentVisitor {
  constructor(options) {
    super(options);
    this.element = new HeaderContentElement();
  }
}
export default ContentVisitor;