"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class OperationReply extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'operationReply';
  }
  get address() {
    return this.get('address');
  }
  set address(address) {
    this.set('address', address);
  }
  get channel() {
    return this.get('channel');
  }
  set channel(channel) {
    this.set('channel', channel);
  }
  get messages() {
    return this.get('messages');
  }
  set messages(messages) {
    this.set('messages', messages);
  }
}
var _default = exports.default = OperationReply;