import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import ServerVariableElement from "../../../../elements/ServerVariable.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ServerVariableVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ServerVariableElement();
    this.specPath = always(['document', 'objects', 'ServerVariable']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default ServerVariableVisitor;