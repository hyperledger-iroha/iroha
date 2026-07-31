"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Parameter extends _apidomCore.ObjectElement {
  constructor(content = {}, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'parameter';
  }
  get enum() {
    return this.get('enum');
  }
  set enum(value) {
    this.set('enum', value);
  }
  get default() {
    return this.get('default');
  }
  set default(value) {
    this.set('default', value);
  }
  get description() {
    return this.get('description');
  }
  set description(value) {
    this.set('description', value);
  }
  get examples() {
    return this.get('examples');
  }
  set examples(value) {
    this.set('examples', value);
  }
  get location() {
    return this.get('location');
  }
  set location(value) {
    this.set('location', value);
  }
}
var _default = exports.default = Parameter;