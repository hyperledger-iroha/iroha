import { always } from 'ramda';
import { Mixin } from 'ts-mixer';
import InfoElement from "../../../../elements/Info.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class InfoVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.specPath = always(['document', 'objects', 'Info']);
    this.element = new InfoElement();
  }
}
export default InfoVisitor;