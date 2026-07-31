import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import SecurityRequirementElement from "../../../../elements/SecurityRequirement.mjs";
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SecurityRequirementVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new SecurityRequirementElement();
    this.specPath = always(['value']);
  }
}
export default SecurityRequirementVisitor;