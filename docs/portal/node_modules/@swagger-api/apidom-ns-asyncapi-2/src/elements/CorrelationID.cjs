"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class CorrelationID extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'correlationID';
  }
  get description() {
    return this.get('description');
  }
  set description(description) {
    this.set('description', description);
  }
  get location() {
    return this.get('location');
  }
  set location(location) {
    this.set('location', location);
  }
}
var _default = exports.default = CorrelationID;