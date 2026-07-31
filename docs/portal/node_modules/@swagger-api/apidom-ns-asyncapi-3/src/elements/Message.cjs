"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _apidomError = require("@swagger-api/apidom-error");
/* eslint-disable class-methods-use-this */

/**
 * @public
 */
class Message extends _apidomNsAsyncapi.MessageElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'message';
  }
  get messageId() {
    throw new _apidomError.UnsupportedOperationError('messageId has been removed');
  }
  set messageId(messageId) {
    throw new _apidomError.UnsupportedOperationError('messageId has been removed');
  }
  get headers() {
    return this.get('headers');
  }
  set headers(headers) {
    this.set('headers', headers);
  }
  get payload() {
    return this.get('payload');
  }
  set payload(payload) {
    this.set('payload', payload);
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }
}
var _default = exports.default = Message;