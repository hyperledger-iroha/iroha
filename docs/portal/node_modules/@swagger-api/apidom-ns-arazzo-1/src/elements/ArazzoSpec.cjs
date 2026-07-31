"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ArazzoSpec extends _apidomCore.StringElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'arazzoSpec';
    this.classes.push('spec-version');
    this.classes.push('version');
  }
}
var _default = exports.default = ArazzoSpec;