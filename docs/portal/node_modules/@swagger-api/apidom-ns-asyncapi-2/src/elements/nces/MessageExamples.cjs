"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class MessageExamples extends _apidomCore.ArrayElement {
  static primaryClass = 'message-examples';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(MessageExamples.primaryClass);
  }
}
var _default = exports.default = MessageExamples;