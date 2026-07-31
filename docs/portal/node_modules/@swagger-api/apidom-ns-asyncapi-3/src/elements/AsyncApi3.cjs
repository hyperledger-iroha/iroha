"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomError = require("@swagger-api/apidom-error");
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
/* eslint-disable class-methods-use-this */

/**
 * @public
 */
class AsyncApi3 extends _apidomNsAsyncapi.AsyncApi2Element {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'asyncApi3';
  }
  get tags() {
    throw new _apidomError.UnsupportedOperationError('tags keyword has been moved to info');
  }
  set tags(tags) {
    throw new _apidomError.UnsupportedOperationError('tags keyword has been moved to info');
  }
  get externalDocs() {
    throw new _apidomError.UnsupportedOperationError('externalDocs keyword has been moved to info.');
  }
  set externalDocs(externalDocs) {
    throw new _apidomError.UnsupportedOperationError('externalDocs keyword has been moved to info.');
  }
  get operations() {
    return this.get('operations');
  }
  set operations(operations) {
    this.set('operations', operations);
  }
}
var _default = exports.default = AsyncApi3;