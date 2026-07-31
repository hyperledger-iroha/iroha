import { StringElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class RequirementLevel extends StringElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'requirementLevel';
  }
}
export default RequirementLevel;