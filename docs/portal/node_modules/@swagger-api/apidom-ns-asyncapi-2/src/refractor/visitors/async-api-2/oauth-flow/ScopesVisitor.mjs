import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import OAuthFlowScopesElement from "../../../../elements/nces/OAuthFlowScopes.mjs";
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
    this.element = new OAuthFlowScopesElement();
    this.specPath = always(['value']);
  }
}
export default ScopesVisitor;