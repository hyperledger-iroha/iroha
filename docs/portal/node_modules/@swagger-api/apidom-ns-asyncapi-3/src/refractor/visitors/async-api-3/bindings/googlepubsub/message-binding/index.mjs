import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import GooglepubsubMessageBindingElement from "../../../../../../elements/bindings/googlepubsub/GooglepubsubMessageBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class GooglepubsubMessageBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new GooglepubsubMessageBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'googlepubusb', 'MessageBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default GooglepubsubMessageBindingVisitor;