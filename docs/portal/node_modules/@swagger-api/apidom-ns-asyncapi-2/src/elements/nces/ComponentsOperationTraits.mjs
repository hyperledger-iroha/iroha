import { ObjectElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class ComponentsOperationTraits extends ObjectElement {
  static primaryClass = 'components-operation-traits';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsOperationTraits.primaryClass);
  }
}
export default ComponentsOperationTraits;