"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class AmqpMessageBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'amqpMessageBinding';
    this.classes.push('message-binding');
  }
  get contentEncoding() {
    return this.get('contentEncoding');
  }
  set contentEncoding(contentEncoding) {
    this.set('contentEncoding', contentEncoding);
  }
  get messageType() {
    return this.get('messageType');
  }
  set messageType(messageType) {
    this.set('messageType', messageType);
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
}
var _default = exports.default = AmqpMessageBinding;