"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Reference extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'reference';
    this.classes.push('json-reference');
    this.classes.push('asyncapi-reference');
  }
  set $ref($ref) {
    this.set('$ref', $ref);
  }
  get $ref() {
    return this.get('$ref');
  }
}
var _default = exports.default = Reference;