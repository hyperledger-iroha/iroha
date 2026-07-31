"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ExternalDocumentation extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'externalDocumentation';
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get url() {
    return this.get('url');
  }
  set url(url) {
    this.set('url', url);
  }
}
var _default = exports.default = ExternalDocumentation;