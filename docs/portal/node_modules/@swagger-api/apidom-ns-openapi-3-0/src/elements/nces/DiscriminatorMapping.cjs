"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class DiscriminatorMapping extends _apidomCore.ObjectElement {
  static primaryClass = 'discriminator-mapping';
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.classes.push(DiscriminatorMapping.primaryClass);
  }
}
var _default = exports.default = DiscriminatorMapping;