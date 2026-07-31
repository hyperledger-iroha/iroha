"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ComponentsTags extends _apidomCore.ObjectElement {
  static primaryClass = 'components-tags';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsTags.primaryClass);
  }
}
var _default = exports.default = ComponentsTags;