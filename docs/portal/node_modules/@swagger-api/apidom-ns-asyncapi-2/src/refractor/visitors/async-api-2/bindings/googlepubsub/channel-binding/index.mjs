import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import GooglepubsubChannelBindingElement from "../../../../../../elements/bindings/googlepubsub/GooglepubsubChannelBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class GooglepubsubChannelBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new GooglepubsubChannelBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'googlepubsub', 'ChannelBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default GooglepubsubChannelBindingVisitor;