"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class OperationMessageMap extends _apidomCore.ObjectElement {
  static primaryClass = 'operation-message-map';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationMessageMap.primaryClass);
  }
  get oneOf() {
    return this.get('oneOf');
  }
  set oneOf(oneOf) {
    this.set('oneOf', oneOf);
  }
}
var _default = exports.default = OperationMessageMap;