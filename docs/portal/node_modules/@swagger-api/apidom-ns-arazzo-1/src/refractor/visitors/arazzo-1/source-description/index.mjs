import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import SourceDescriptionElement from "../../../../elements/SourceDescription.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SourceDescriptionVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new SourceDescriptionElement();
    this.specPath = always(['document', 'objects', 'SourceDescription']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default SourceDescriptionVisitor;