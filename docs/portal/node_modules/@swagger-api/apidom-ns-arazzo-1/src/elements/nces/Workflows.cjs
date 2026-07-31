"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Workflows extends _apidomCore.ArrayElement {
  static primaryClass = 'workflows';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(Workflows.primaryClass);
  }
}
var _default = exports.default = Workflows;