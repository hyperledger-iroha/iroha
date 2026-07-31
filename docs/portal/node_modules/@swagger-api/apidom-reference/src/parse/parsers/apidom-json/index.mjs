import { ParseResultElement, isParseResultElement, namespace as baseNamespace } from '@swagger-api/apidom-core';
import ParserError from "../../../errors/ParserError.mjs";
import Parser from "../Parser.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ApiDOMJSONParser extends Parser {
  namespace;
  ['apidom-json'];
  constructor(options) {
    const {
      fileExtensions = [],
      mediaTypes = ['application/vnd.apidom', 'application/vnd.apidom+json'],
      namespace = baseNamespace,
      ...rest
    } = options !== null && options !== void 0 ? options : {};
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
    const namespace = (_this$apidomJson$nam = (_this$apidomJson = this['apidom-json']) === null || _this$apidomJson === void 0 ? void 0 : _this$apidomJson.namespace) !== null && _this$apidomJson$nam !== void 0 ? _this$apidomJson$nam : this.namespace;

    // allow empty files
    if (this.allowEmpty && source.trim() === '') {
      return new ParseResultElement();
    }
    try {
      const element = namespace.fromRefract(JSON.parse(source));
      if (!isParseResultElement(element)) {
        element.classes.push('result');
        return new ParseResultElement([element]);
      }
      return element;
    } catch (error) {
      throw new ParserError(`Error parsing "${file.uri}"`, {
        cause: error
      });
    }
  }
}
export default ApiDOMJSONParser;