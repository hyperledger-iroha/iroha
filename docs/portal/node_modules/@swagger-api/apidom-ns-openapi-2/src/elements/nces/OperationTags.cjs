"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class OperationTags extends _apidomCore.ArrayElement {
  static primaryClass = 'operation-tags';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(OperationTags.primaryClass);
    this.classes.push('tags');
  }
}
var _default = exports.default = OperationTags;