"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class MultiFormatSchema extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'multiFormatSchema';
  }
  get schemaFormat() {
    return this.get('schemaFormat');
  }
  set schemaFormat(schemaFormat) {
    this.set('schemaFormat', schemaFormat);
  }
  get schema() {
    return this.get('schema');
  }
  set schema(schema) {
    this.set('schema', schema);
  }
}
var _default = exports.default = MultiFormatSchema;