import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class MqttChannelBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'mqttChannelBinding';
    this.classes.push('channel-binding');
  }
}
export default MqttChannelBinding;