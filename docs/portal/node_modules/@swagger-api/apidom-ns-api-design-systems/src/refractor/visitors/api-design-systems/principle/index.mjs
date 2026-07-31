import { always } from 'ramda';
import { Mixin } from 'ts-mixer';
import PrincipleElement from "../../../../elements/Principle.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class PrincipleVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new PrincipleElement();
    this.specPath = always(['document', 'objects', 'Principle']);
  }
}
export default PrincipleVisitor;