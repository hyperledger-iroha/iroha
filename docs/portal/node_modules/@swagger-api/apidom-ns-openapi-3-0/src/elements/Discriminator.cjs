"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class Discriminator extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'discriminator';
  }
  get propertyName() {
    return this.get('propertyName');
  }
  set propertyName(propertyName) {
    this.set('propertyName', propertyName);
  }
  get mapping() {
    return this.get('mapping');
  }
  set mapping(mapping) {
    this.set('mapping', mapping);
  }
}
var _default = exports.default = Discriminator;