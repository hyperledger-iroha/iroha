"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class OperationReplyAddress extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'operationReplyAddress';
  }
  get description() {
    return this.get('description');
  }
  set description(value) {
    this.set('description', value);
  }
  get location() {
    return this.get('location');
  }
  set location(value) {
    this.set('location', value);
  }
}
var _default = exports.default = OperationReplyAddress;