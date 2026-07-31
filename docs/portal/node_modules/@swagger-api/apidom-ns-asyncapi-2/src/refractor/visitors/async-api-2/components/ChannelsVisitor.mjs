import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
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
    this.specPath = always(['document', 'objects', 'ChannelItem']);
  }
}
export default ChannelsVisitor;