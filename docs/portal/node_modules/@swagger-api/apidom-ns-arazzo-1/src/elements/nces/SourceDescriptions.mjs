import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class SourceDescriptions extends ArrayElement {
  static primaryClass = 'sourceDescriptions';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(SourceDescriptions.primaryClass);
  }
}
export default SourceDescriptions;