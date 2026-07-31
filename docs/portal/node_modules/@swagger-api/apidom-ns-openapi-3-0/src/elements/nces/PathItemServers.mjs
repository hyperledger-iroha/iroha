import { ArrayElement } from '@swagger-api/apidom-core';

/**
 * @public
 */
class PathItemServers extends ArrayElement {
  static primaryClass = 'path-item-servers';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(PathItemServers.primaryClass);
    this.classes.push('servers');
  }
}
export default PathItemServers;