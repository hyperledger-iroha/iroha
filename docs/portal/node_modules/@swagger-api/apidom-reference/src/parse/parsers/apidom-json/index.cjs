"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _ParserError = _interopRequireDefault(require("../../../errors/ParserError.cjs"));
var _Parser = _interopRequireDefault(require("../Parser.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class ApiDOMJSONParser extends _Parser.default {
  namespace;
  ['apidom-json'];
  constructor(options) {
    const {
      fileExtensions = [],
      mediaTypes = ['application/vnd.apidom', 'application/vnd.apidom+json'],
      namespace = _apidomCore.namespace,
      ...rest
    } = options != null ? options : {};
    super({
      ...rest,
      name: 'apidom-json',
      fileExtensions,
      mediaTypes
    });
    this.namespace = namespace;
  }
  canParse(file) {
    const hasSupportedFileExtension = this.fileExtensions.length === 0 ? true : this.fileExtensions.includes(file.extension);
    const hasSupportedMediaType = this.mediaTypes.includes(file.mediaType);
    if (!hasSupportedFileExtension) return false;
    if (hasSupportedMediaType) return true;
    if (!hasSupportedMediaType) {
      try {
        return this.namespace.fromRefract(JSON.parse(file.toString())) && true;
      } catch {
        return false;
      }
    }
    return false;
  }
  parse(file) {
    var _this$apidomJson$nam, _this$apidomJson;
    const source = file.toString();
    const namespace = (_this$apidomJson$nam = (_this$apidomJson = this['apidom-json']) == null ? void 0 : _this$apidomJson.namespace) != null ? _this$apidomJson$nam : this.namespace;

    // allow empty files
    if (this.allowEmpty && source.trim() === '') {
      return new _apidomCore.ParseResultElement();
    }
    try {
      const element = namespace.fromRefract(JSON.parse(source));
      if (!(0, _apidomCore.isParseResultElement)(element)) {
        element.classes.push('result');
        return new _apidomCore.ParseResultElement([element]);
      }
      return element;
    } catch (error) {
      throw new _ParserError.default(`Error parsing "${file.uri}"`, {
        cause: error
      });
    }
  }
}
var _default = exports.default = ApiDOMJSONParser;