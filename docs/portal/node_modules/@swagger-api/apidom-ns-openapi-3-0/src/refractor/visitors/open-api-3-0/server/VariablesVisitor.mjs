import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import ServerVariablesElement from "../../../../elements/nces/ServerVariables.mjs";
/**
 * @public
 */
/**
 * @public
 */
class VariablesVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ServerVariablesElement();
    this.specPath = always(['document', 'objects', 'ServerVariable']);
  }
}
export default VariablesVisitor;