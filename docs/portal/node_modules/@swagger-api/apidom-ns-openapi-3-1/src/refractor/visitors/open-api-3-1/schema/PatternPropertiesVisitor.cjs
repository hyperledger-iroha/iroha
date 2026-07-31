"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomNsJsonSchema = require("@swagger-api/apidom-ns-json-schema-2020-12");
/**
 * @public
 */
class PatternPropertiesVisitor extends _apidomNsJsonSchema.PatternPropertiesVisitor {
  constructor(options) {
    super(options);
    this.passingOptionsNames.push('parent');
  }
}
var _default = exports.default = PatternPropertiesVisitor;