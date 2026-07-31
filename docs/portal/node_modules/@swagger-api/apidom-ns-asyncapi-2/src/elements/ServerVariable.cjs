"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ServerVariable extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'serverVariable';
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
  set description(description) {
    this.set('description', description);
  }
  get examples() {
    return this.get('examples');
  }
  set examples(examples) {
    this.set('examples', examples);
  }
}
var _default = exports.default = ServerVariable;