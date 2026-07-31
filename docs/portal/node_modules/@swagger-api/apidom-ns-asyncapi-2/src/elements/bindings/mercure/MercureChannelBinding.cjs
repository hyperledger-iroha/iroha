"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class MercureChannelBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'mercureChannelBinding';
    this.classes.push('channel-binding');
  }
}
var _default = exports.default = MercureChannelBinding;