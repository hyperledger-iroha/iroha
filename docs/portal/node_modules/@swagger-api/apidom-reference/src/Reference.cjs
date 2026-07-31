"use strict";

exports.__esModule = true;
exports.default = void 0;
/**
 * @public
 */

/**
 * @public
 */
class Reference {
  uri;
  depth;
  value;
  refSet;
  errors;
  constructor({
    uri,
    depth = 0,
    refSet,
    value
  }) {
    this.uri = uri;
    this.value = value;
    this.depth = depth;
    this.refSet = refSet;
    this.errors = [];
  }
}
var _default = exports.default = Reference;