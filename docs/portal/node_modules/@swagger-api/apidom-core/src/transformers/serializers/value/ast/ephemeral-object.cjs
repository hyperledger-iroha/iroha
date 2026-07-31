"use strict";

exports.__esModule = true;
exports.default = void 0;
class EphemeralObject {
  type = 'EphemeralObject';
  content = [];
  reference = undefined;
  constructor(content) {
    this.content = content;
    this.reference = {};
  }
  toReference() {
    return this.reference;
  }
  toObject() {
    return Object.assign(this.reference, Object.fromEntries(this.content));
  }
}
var _default = exports.default = EphemeralObject;