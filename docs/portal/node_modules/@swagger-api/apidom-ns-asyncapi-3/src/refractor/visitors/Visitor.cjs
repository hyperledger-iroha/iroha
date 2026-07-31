"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
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
      to.meta = (0, _apidomCore.deepmerge)(to.meta, from.meta);
    }
    if ((0, _apidomCore.hasElementSourceMap)(from)) {
      (0, _apidomCore.assignSourceMap)(to, from);
    }
    if (from.attributes.length > 0 || from.meta.length > 0) {
      to.attributes = (0, _apidomCore.deepmerge)(to.attributes, from.attributes); // eslint-disable-line no-param-reassign
    }
  }
  /* eslint-enable class-methods-use-this, no-param-reassign */
}
var _default = exports.default = Visitor;