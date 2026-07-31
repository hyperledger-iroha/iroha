import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class WebSocketOperationBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'webSocketOperationBinding';
    this.classes.push('operation-binding');
  }
}
export default WebSocketOperationBinding;