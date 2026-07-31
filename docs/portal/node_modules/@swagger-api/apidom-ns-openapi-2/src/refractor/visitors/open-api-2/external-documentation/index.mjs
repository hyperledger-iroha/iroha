import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import ExternalDocumentationElement from "../../../../elements/ExternalDocumentation.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ExternalDocumentationVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ExternalDocumentationElement();
    this.specPath = always(['document', 'objects', 'ExternalDocumentation']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default ExternalDocumentationVisitor;