import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import MapVisitor from "../../generics/MapVisitor.mjs";
import FallbackVisitor from "../../FallbackVisitor.mjs";
import ChannelsElement from "../../../../elements/Channels.mjs";
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
    this.specPath = always(['document', 'objects', 'ChannelItem']);
  }
}
export default ChannelsVisitor;