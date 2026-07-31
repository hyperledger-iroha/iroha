"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsAsyncapi = require("@swagger-api/apidom-ns-asyncapi-2");
var _apidomError = require("@swagger-api/apidom-error");
/* eslint-disable class-methods-use-this */

/**
 * @public
 */
class Server extends _apidomNsAsyncapi.ServerElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'server';
  }
  get url() {
    throw new _apidomError.UnsupportedOperationError('url keyword from Core vocabulary has been removed');
  }
  set url(url) {
    throw new _apidomError.UnsupportedOperationError('url keyword from Core vocabulary has been removed');
  }
  get host() {
    return this.get('host');
  }
  set host(host) {
    this.set('host', host);
  }
  get pathName() {
    return this.get('pathName');
  }
  set pathName(pathName) {
    this.set('pathName', pathName);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get title() {
    return this.get('title');
  }
  set title(title) {
    this.set('title', title);
  }
  get summary() {
    return this.get('summary');
  }
  set summary(summary) {
    this.set('summary', summary);
  }
  get externalDocs() {
    return this.get('externalDocs');
  }
  set externalDocs(externalDocs) {
    this.set('externalDocs', externalDocs);
  }
}
var _default = exports.default = Server;