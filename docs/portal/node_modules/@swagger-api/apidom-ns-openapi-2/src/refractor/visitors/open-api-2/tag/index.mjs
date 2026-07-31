import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import TagElement from "../../../../elements/Tag.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class TagVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new TagElement();
    this.specPath = always(['document', 'objects', 'Tag']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default TagVisitor;