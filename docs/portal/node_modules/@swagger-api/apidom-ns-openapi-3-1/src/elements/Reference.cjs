"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
/**
 * @public
 */
class Reference extends _apidomNsOpenapi.ReferenceElement {}
Object.defineProperty(Reference.prototype, 'description', {
  get() {
    return this.get('description');
  },
  set(description) {
    this.set('description', description);
  },
  enumerable: true
});
Object.defineProperty(Reference.prototype, 'summary', {
  get() {
    return this.get('summary');
  },
  set(description) {
    this.set('summary', description);
  },
  enumerable: true
});
var _default = exports.default = Reference;