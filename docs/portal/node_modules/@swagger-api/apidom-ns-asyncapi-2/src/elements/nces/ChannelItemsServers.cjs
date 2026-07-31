"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ChannelItemServers extends _apidomCore.ArrayElement {
  static primaryClass = 'channel-item-server-names-list';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ChannelItemServers.primaryClass);
  }
}
var _default = exports.default = ChannelItemServers;