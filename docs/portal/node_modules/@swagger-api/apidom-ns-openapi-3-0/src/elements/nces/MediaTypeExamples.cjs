"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class MediaTypeExamples extends _apidomCore.ObjectElement {
  static primaryClass = 'media-type-examples';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(MediaTypeExamples.primaryClass);
    this.classes.push('examples');
  }
}
var _default = exports.default = MediaTypeExamples;