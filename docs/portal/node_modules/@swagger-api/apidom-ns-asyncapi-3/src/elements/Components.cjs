"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
/**
 * @public
 */
class Components extends _apidomNsAsyncapi.ComponentsElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'components';
  }
  get operations() {
    return this.get('operations');
  }
  set operations(operations) {
    this.set('operations', operations);
  }
  get replies() {
    return this.get('replies');
  }
  set replies(replies) {
    this.set('replies', replies);
  }
  get replyAddresses() {
    return this.get('replyAddresses');
  }
  set replyAddresses(replyAddresses) {
    this.set('replyAddresses', replyAddresses);
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }
  get tags() {
    return this.get('tags');
  }
  set tags(tags) {
    this.set('tags', tags);
  }
}
var _default = exports.default = Components;