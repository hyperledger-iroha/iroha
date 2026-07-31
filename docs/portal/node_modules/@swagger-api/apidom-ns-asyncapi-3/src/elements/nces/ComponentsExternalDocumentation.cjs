"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class ComponentsExternalDocumentation extends _apidomCore.ObjectElement {
  static primaryClass = 'components-external-documentation';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(ComponentsExternalDocumentation.primaryClass);
  }
}
var _default = exports.default = ComponentsExternalDocumentation;