"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class OperationReplyMessages extends _apidomCore.ArrayElement {
  static primaryClass = 'operation-reply-messages';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationReplyMessages.primaryClass);
  }
}
var _default = exports.default = OperationReplyMessages;