"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _apidomParserAdapterOpenapiJson = require("@swagger-api/apidom-parser-adapter-openapi-json-3-1");
var _ParserError = _interopRequireDefault(require("../../../errors/ParserError.cjs"));
var _Parser = _interopRequireDefault(require("../Parser.cjs"));
/**
 * @public
 */
/* eslint-disable @typescript-eslint/naming-convention */

/**
 * @public
 */
class OpenAPIJSON3_1Parser extends _Parser.default {
  syntacticAnalysis;
  refractorOpts;
  constructor(options) {
    const {
      fileExtensions = [],
      mediaTypes = _apidomParserAdapterOpenapiJson.mediaTypes,
      ...rest
    } = options != null ? options : {};
    super({
      ...rest,
      name: 'openapi-json-3-1',
      fileExtensions,
      mediaTypes
    });
  }
  async canParse(file) {
    const hasSupportedFileExtension = this.fileExtensions.length === 0 ? true : this.fileExtensions.includes(file.extension);
    const hasSupportedMediaType = this.mediaTypes.includes(file.mediaType);
    if (!hasSupportedFileExtension) return false;
    if (hasSupportedMediaType) return true;
    if (!hasSupportedMediaType) {
      return (0, _apidomParserAdapterOpenapiJson.detect)(file.toString());
    }
    return false;
  }
  async parse(file) {
    const source = file.toString();
    try {
      const parserOpts = (0, _ramda.pick)(['sourceMap', 'syntacticAnalysis', 'refractorOpts'], this);
      return await (0, _apidomParserAdapterOpenapiJson.parse)(source, parserOpts);
    } catch (error) {
      throw new _ParserError.default(`Error parsing "${file.uri}"`, {
        cause: error
      });
    }
  }
}
/* eslint-enable @typescript-eslint/naming-convention */
var _default = exports.default = OpenAPIJSON3_1Parser;