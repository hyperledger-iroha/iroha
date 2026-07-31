"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Scenario extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'scenario';
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get when() {
    return this.get('when');
  }
  set when(when) {
    this.set('when', when);
  }
  get then() {
    return this.get('then');
  }
  set then(then) {
    this.set('then', then);
  }
}
var _default = exports.default = Scenario;