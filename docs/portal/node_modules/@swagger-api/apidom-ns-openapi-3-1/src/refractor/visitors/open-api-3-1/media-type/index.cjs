"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = exports.BaseMediaTypeVisitor = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-0");
var _MediaType = _interopRequireDefault(require("../../../../elements/MediaType.cjs"));
/**
 * @public
 */
const BaseMediaTypeVisitor = exports.BaseMediaTypeVisitor = _apidomNsOpenapi.specificationObj.visitors.document.objects.MediaType.$visitor;
/**
 * @public
 */
class MediaTypeVisitor extends BaseMediaTypeVisitor {
  constructor(options) {
    super(options);
    this.element = new _MediaType.default();
  }
}
var _default = exports.default = MediaTypeVisitor;