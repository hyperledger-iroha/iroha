import { always } from 'ramda';
import { Mixin } from 'ts-mixer';
import StandardElement from "../../../../elements/Standard.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class StandardVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new StandardElement();
    this.specPath = always(['document', 'objects', 'Standard']);
  }
}
export default StandardVisitor;