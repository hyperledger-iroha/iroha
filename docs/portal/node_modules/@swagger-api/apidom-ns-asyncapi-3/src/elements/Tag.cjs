"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
/**
 * @public
 */
class Tag extends _apidomNsAsyncapi.TagElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'tag';
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }
}
var _default = exports.default = Tag;