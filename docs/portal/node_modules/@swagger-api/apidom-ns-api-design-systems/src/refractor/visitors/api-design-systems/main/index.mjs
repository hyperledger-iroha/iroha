import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import MainElement from "../../../../elements/Main.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MainVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new MainElement();
    this.specPath = always(['document', 'objects', 'Main']);
  }
}
export default MainVisitor;