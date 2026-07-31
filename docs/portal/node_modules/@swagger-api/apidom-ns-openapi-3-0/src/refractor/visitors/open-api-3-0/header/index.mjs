import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import HeaderElement from "../../../../elements/Header.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class HeaderVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new HeaderElement();
    this.specPath = always(['document', 'objects', 'Header']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default HeaderVisitor;