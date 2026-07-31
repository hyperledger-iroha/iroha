import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import SecurityDefinitionsElement from "../../../../elements/SecurityDefinitions.mjs";
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SecurityDefinitionsVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  element;
  constructor(options) {
    super(options);
    this.element = new SecurityDefinitionsElement();
    this.specPath = always(['document', 'objects', 'SecurityScheme']);
  }
}
export default SecurityDefinitionsVisitor;