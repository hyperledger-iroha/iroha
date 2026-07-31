import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import DiscriminatorElement from "../../../../elements/Discriminator.mjs";
import FixedFieldsVisitor from "../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class DiscriminatorVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new DiscriminatorElement();
    this.specPath = always(['document', 'objects', 'Discriminator']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default DiscriminatorVisitor;