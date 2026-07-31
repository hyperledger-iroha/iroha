import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import ComponentsParametersElement from "../../../../elements/nces/ComponentsParameters.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import MapVisitor from "../../generics/MapVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ParametersVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ComponentsParametersElement();
    this.specPath = always(['document', 'objects', 'Parameter']);
  }
}
export default ParametersVisitor;