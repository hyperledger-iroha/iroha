import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import LinkParametersElement from "../../../../elements/nces/LinkParameters.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ParametersVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new LinkParametersElement();
    this.specPath = always(['value']);
  }
}
export default ParametersVisitor;