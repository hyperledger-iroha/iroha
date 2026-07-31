"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class StompMessageBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'stompMessageBinding';
    this.classes.push('message-binding');
  }
}
var _default = exports.default = StompMessageBinding;