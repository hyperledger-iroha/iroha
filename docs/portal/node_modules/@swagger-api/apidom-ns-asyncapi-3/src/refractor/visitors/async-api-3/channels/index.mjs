import { Mixin } from 'ts-mixer';
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import ChannelsElement from "../../../../elements/Channels.mjs";
import { isReferenceElement } from "../../../../predicates.mjs";
import { isReferenceLikeElement } from "../../../predicates.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ChannelsVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ChannelsElement();
    this.specPath = element => isReferenceLikeElement(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Channel'];
    this.canSupportSpecificationExtensions = false;
  }
  ObjectElement(objectElement) {
    const result = MapVisitor.prototype.ObjectElement.call(this, objectElement);

    // @ts-ignore
    this.element.filter(isReferenceElement).forEach(referenceElement => {
      referenceElement.setMetaProperty('referenced-element', 'channel');
    });
    return result;
  }
}
export default ChannelsVisitor;