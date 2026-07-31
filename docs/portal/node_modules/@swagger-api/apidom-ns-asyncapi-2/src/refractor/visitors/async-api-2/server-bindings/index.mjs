import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import ServerBindingsElement from "../../../../elements/ServerBindings.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ServerBindingsVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ServerBindingsElement();
    this.specPath = always(['document', 'objects', 'ServerBindings']);
    this.canSupportSpecificationExtensions = true;
  }
}
export default ServerBindingsVisitor;