"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Server extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'server';
  }
  get url() {
    return this.get('url');
  }
  set url(url) {
    this.set('url', url);
  }
  get protocol() {
    return this.get('protocol');
  }
  set protocol(protocol) {
    this.set('protocol', protocol);
  }
  get protocolVersion() {
    return this.get('protocolVersion');
  }
  set protocolVersion(protocolVersion) {
    this.set('protocolVersion', protocolVersion);
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get variables() {
    return this.get('variables');
  }
  set variables(variables) {
    this.set('variables', variables);
  }
  get tags() {
    return this.get('tags');
  }
  set tags(tags) {
    this.set('tags', tags);
  }
  get security() {
    return this.get('security');
  }
  set security(security) {
    this.set('security', security);
  }
  get bindings() {
    return this.get('bindings');
  }
  set bindings(bindings) {
    this.set('bindings', bindings);
  }
}
var _default = exports.default = Server;