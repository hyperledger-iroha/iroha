"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class RequirementLevel extends _apidomCore.StringElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'requirementLevel';
  }
}
var _default = exports.default = RequirementLevel;