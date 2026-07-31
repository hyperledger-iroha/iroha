"use strict";

exports.__esModule = true;
exports.default = void 0;
var _minim = require("minim");
/**
 * @public
 */
class Annotation extends _minim.StringElement {
  // classes: warning | error

  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'annotation';
  }
  get code() {
    return this.attributes.get('code');
  }
  set code(value) {
    this.attributes.set('code', value);
  }
}
var _default = exports.default = Annotation;