"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Identifier extends _apidomCore.StringElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'identifier';
  }
}
var _default = exports.default = Identifier;