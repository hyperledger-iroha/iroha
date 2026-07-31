"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ComponentsParameters extends _apidomCore.ObjectElement {
  static primaryClass = 'components-parameters';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsParameters.primaryClass);
    this.classes.push('parameters');
  }
}
var _default = exports.default = ComponentsParameters;