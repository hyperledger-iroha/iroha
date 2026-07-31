"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Webhooks extends _apidomCore.ObjectElement {
  static primaryClass = 'webhooks';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(Webhooks.primaryClass);
  }
}
var _default = exports.default = Webhooks;