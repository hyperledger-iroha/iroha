"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * URI: https://datatracker.ietf.org/doc/html/draft-wright-json-schema-hyperschema-00#section-4.3
 * @public
 */

class Media extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'media';
  }
  get binaryEncoding() {
    return this.get('binaryEncoding');
  }
  set binaryEncoding(binaryEncoding) {
    this.set('binaryEncoding', binaryEncoding);
  }
  get type() {
    return this.get('type');
  }
  set type(type) {
    this.set('type', type);
  }
}
var _default = exports.default = Media;