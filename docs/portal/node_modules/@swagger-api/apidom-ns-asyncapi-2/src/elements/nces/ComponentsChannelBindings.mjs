import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsChannelBindings extends ObjectElement {
  static primaryClass = 'components-channel-bindings';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsChannelBindings.primaryClass);
  }
}
export default ComponentsChannelBindings;