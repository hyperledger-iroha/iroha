import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import MediaTypeElement from "../../../../elements/MediaType.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MediaTypeVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new MediaTypeElement();
    this.specPath = always(['document', 'objects', 'MediaType']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default MediaTypeVisitor;