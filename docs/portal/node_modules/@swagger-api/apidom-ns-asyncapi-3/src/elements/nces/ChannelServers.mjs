import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ChannelServers extends ArrayElement {
  static primaryClass = 'channel-servers';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ChannelServers.primaryClass);
  }
}
export default ChannelServers;