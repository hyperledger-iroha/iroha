"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ChannelServers extends _apidomCore.ArrayElement {
  static primaryClass = 'channel-servers';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ChannelServers.primaryClass);
  }
}
var _default = exports.default = ChannelServers;