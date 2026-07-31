import { Mixin } from 'ts-mixer';
import { isReferenceElement, isReferenceLikeElement } from '@swagger-api/apidom-ns-asyncapi-2';
import ComponentsChannelsElement from "../../../../elements/nces/ComponentsChannels.mjs";
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ChannelsVisitor extends Mixin(MapVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new ComponentsChannelsElement();
    this.specPath = element => isReferenceLikeElement(element) ? ['document', 'objects', 'Reference'] : ['document', 'objects', 'Channel'];
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