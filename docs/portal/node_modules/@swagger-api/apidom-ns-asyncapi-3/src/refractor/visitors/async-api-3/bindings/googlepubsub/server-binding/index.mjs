import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import GooglepubsubServerBindingElement from "../../../../../../elements/bindings/googlepubsub/GooglepubsubServerBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class GooglepubsubServerBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new GooglepubsubServerBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'googlepubsub', 'ServerBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default GooglepubsubServerBindingVisitor;