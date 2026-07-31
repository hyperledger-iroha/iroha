"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _apidomError = require("@swagger-api/apidom-error");
/* eslint-disable class-methods-use-this */

/**
 * @public
 */
class OperationTrait extends _apidomNsAsyncapi.OperationTraitElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'operationTrait';
  }
  get operationId() {
    throw new _apidomError.UnsupportedOperationError('operationId keyword from Core vocabulary has been removed');
  }
  set operationId(operationId) {
    throw new _apidomError.UnsupportedOperationError('operationId keyword from Core vocabulary has been removed');
  }
  get title() {
    return this.get('title');
  }
  set title(title) {
    this.set('title', title);
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }
}
var _default = exports.default = OperationTrait;