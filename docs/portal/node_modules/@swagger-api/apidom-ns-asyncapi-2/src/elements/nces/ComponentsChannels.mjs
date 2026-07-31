import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsChannels extends ObjectElement {
  static primaryClass = 'components-channels';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsChannels.primaryClass);
  }
}
export default ComponentsChannels;