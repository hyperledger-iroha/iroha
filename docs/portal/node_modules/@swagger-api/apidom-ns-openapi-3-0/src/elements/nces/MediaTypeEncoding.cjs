"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class MediaTypeEncoding extends _apidomCore.ObjectElement {
  static primaryClass = 'media-type-encoding';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(MediaTypeEncoding.primaryClass);
  }
}
var _default = exports.default = MediaTypeEncoding;