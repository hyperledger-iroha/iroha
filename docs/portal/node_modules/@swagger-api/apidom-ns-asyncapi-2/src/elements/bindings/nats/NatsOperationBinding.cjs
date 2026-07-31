"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class NatsOperationBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'natsOperationBinding';
    this.classes.push('operation-binding');
  }
  get queue() {
    return this.get('queue');
  }
  set queue(queue) {
    this.set('queue', queue);
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
}
var _default = exports.default = NatsOperationBinding;