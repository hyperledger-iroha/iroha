"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Amqp1OperationBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'amqp1OperationBinding';
    this.classes.push('operation-binding');
  }
}
var _default = exports.default = Amqp1OperationBinding;