import { always } from 'ramda';
import { Mixin } from 'ts-mixer';
import RequirementElement from "../../../../elements/Requirement.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class RequirementVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new RequirementElement();
    this.specPath = always(['document', 'objects', 'Requirement']);
  }
}
export default RequirementVisitor;