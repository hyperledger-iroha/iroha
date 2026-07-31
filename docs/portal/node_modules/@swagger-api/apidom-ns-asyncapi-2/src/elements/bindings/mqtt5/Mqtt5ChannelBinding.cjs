"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Mqtt5ChannelBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'mqtt5ChannelBinding';
    this.classes.push('channel-binding');
  }
}
var _default = exports.default = Mqtt5ChannelBinding;