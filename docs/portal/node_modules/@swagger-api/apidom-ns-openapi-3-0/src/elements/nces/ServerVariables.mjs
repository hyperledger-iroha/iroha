import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ServerVariables extends ObjectElement {
  static primaryClass = 'server-variables';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ServerVariables.primaryClass);
  }
}
export default ServerVariables;