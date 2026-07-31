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
}
var _default = exports.default = Server;