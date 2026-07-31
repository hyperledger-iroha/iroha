import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import SecuritySchemeElement from "../../../../elements/SecurityScheme.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class SecuritySchemeVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new SecuritySchemeElement();
    this.specPath = always(['document', 'objects', 'SecurityScheme']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default SecuritySchemeVisitor;