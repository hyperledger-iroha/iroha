import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import FailureActionElement from "../../../../elements/FailureAction.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class FailureActionVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new FailureActionElement();
    this.specPath = always(['document', 'objects', 'FailureAction']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default FailureActionVisitor;