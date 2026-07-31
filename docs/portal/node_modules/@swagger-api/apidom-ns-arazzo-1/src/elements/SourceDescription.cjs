"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class SourceDescription extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'sourceDescription';
  }
  get name() {
    return this.get('name');
  }
  set name(name) {
    this.set('name', name);
  }
  get url() {
    return this.get('url');
  }
  set url(url) {
    this.set('url', url);
  }
  get type() {
    return this.get('type');
  }
  set type(type) {
    this.set('type', type);
  }
}
var _default = exports.default = SourceDescription;