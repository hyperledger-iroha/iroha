import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class WebSocketMessageBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'webSocketMessageBinding';
    this.classes.push('message-binding');
  }
}
export default WebSocketMessageBinding;