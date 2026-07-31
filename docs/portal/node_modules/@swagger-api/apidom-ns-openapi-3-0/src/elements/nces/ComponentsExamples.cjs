"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ComponentsExamples extends _apidomCore.ObjectElement {
  static primaryClass = 'components-examples';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsExamples.primaryClass);
    this.classes.push('examples');
  }
}
var _default = exports.default = ComponentsExamples;