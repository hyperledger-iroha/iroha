"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class OperationMessages extends _apidomCore.ArrayElement {
  static primaryClass = 'operation-messages';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationMessages.primaryClass);
  }
}
var _default = exports.default = OperationMessages;