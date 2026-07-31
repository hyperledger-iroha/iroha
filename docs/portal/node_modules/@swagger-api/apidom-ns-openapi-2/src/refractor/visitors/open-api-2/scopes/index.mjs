import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import ScopesElement from "../../../../elements/Scopes.mjs";
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ScopesVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ScopesElement();
    this.specPath = always(['value']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default ScopesVisitor;