import { hasElementSourceMap, deepmerge, assignSourceMap } from '@swagger-api/apidom-core';

/**
 * @public
 */

/**
 * @public
 */
class Visitor {
  element;
  constructor(options) {
    Object.assign(this, options);
  }

  /* eslint-disable class-methods-use-this, no-param-reassign */
  copyMetaAndAttributes(from, to) {
    if (from.meta.length > 0 || to.meta.length > 0) {
      to.meta = deepmerge(to.meta, from.meta);
    }
    if (hasElementSourceMap(from)) {
      assignSourceMap(to, from);
    }
    if (from.attributes.length > 0 || from.meta.length > 0) {
      to.attributes = deepmerge(to.attributes, from.attributes);
    }
  }
  /* eslint-enable class-methods-use-this, no-param-reassign */
}
export default Visitor;