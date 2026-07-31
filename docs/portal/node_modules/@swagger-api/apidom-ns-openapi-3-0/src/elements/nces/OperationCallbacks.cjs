"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class OperationCallbacks extends _apidomCore.ObjectElement {
  static primaryClass = 'operation-callbacks';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationCallbacks.primaryClass);
  }
}
var _default = exports.default = OperationCallbacks;