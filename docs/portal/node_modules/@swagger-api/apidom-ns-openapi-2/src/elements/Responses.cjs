"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Responses extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'responses';
  }
  get default() {
    return this.get('default');
  }
  set default(defaultValue) {
    this.set('default', defaultValue);
  }
}
var _default = exports.default = Responses;