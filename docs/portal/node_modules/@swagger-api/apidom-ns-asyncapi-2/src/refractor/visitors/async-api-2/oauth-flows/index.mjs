import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import OAuthFlowsElement from "../../../../elements/OAuthFlows.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class OAuthFlowsVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new OAuthFlowsElement();
    this.specPath = always(['document', 'objects', 'OAuthFlows']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default OAuthFlowsVisitor;