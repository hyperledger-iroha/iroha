"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class OperationParameters extends _apidomCore.ArrayElement {
  static primaryClass = 'operation-parameters';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationParameters.primaryClass);
    this.classes.push('parameters');
  }
}
var _default = exports.default = OperationParameters;