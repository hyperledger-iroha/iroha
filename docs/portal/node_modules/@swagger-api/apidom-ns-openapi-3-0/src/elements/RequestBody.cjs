"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class RequestBody extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'requestBody';
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get contentProp() {
    return this.get('content');
  }
  set contentProp(content) {
    this.set('content', content);
  }
  get required() {
    if (this.hasKey('required')) {
      return this.get('required');
    }
    return new _apidomCore.BooleanElement(false);
  }
  set required(required) {
    this.set('required', required);
  }
}
var _default = exports.default = RequestBody;