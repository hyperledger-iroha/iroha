"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _apidomParserAdapterArazzoYaml = require("@swagger-api/apidom-parser-adapter-arazzo-yaml-1");
var _ParserError = _interopRequireDefault(require("../../../errors/ParserError.cjs"));
var _Parser = _interopRequireDefault(require("../Parser.cjs"));
/**
 * @public
 */

/**
 * @public
 */
class ArazzoYAML1Parser extends _Parser.default {
  refractorOpts;
  constructor(options) {
    const {
      fileExtensions = [],
      mediaTypes = _apidomParserAdapterArazzoYaml.mediaTypes,
      ...rest
    } = options != null ? options : {};
    super({
      ...rest,
      name: 'arazzo-yaml-1',
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
      return (0, _apidomParserAdapterArazzoYaml.detect)(file.toString());
    }
    return false;
  }
  async parse(file) {
    const source = file.toString();
    try {
      const parserOpts = (0, _ramda.pick)(['sourceMap', 'refractorOpts'], this);
      return await (0, _apidomParserAdapterArazzoYaml.parse)(source, parserOpts);
    } catch (error) {
      throw new _ParserError.default(`Error parsing "${file.uri}"`, {
        cause: error
      });
    }
  }
}
var _default = exports.default = ArazzoYAML1Parser;