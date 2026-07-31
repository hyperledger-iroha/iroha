"use strict";

exports.__esModule = true;
exports.AncestorLineage = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
// eslint-disable-next-line import/prefer-default-export
class AncestorLineage extends Array {
  includesCycle(element) {
    return this.filter(ancestors => ancestors.has(element)).length > 1;
  }
  includes(searchElement, fromIndex) {
    if (searchElement instanceof Set) {
      return super.includes(searchElement, fromIndex);
    }
    return this.some(ancestors => ancestors.has(searchElement));
  }
  findItem(predicate) {
    for (const set of this) {
      for (const item of set) {
        if ((0, _apidomCore.isElement)(item) && predicate(item)) {
          return item;
        }
      }
    }
    return undefined;
  }
}
exports.AncestorLineage = AncestorLineage;