"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomNsOpenapi = require("@swagger-api/apidom-ns-openapi-3-1");
var _BundleStrategy = _interopRequireDefault(require("../BundleStrategy.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class OpenAPI3_1BundleStrategy extends _BundleStrategy.default {
  constructor(options) {
    super({
      ...(options != null ? options : {}),
      name: 'openapi-3-1'
    });
  }
  canBundle(file) {
    var _file$parseResult;
    // assert by media type
    if (file.mediaType !== 'text/plain') {
      return _apidomNsOpenapi.mediaTypes.includes(file.mediaType);
    }

    // assert by inspecting ApiDOM
    return (0, _apidomNsOpenapi.isOpenApi3_1Element)((_file$parseResult = file.parseResult) == null ? void 0 : _file$parseResult.result);
  }
  async bundle(file) {
    return file.parseResult;
  }
}
var _default = exports.default = OpenAPI3_1BundleStrategy;