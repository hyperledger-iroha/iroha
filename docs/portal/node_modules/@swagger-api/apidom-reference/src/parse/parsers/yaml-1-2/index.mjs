import { parse, mediaTypes as YAMLMediaTypes, detect } from '@swagger-api/apidom-parser-adapter-yaml-1-2';
import ParserError from "../../../errors/ParserError.mjs";
import Parser from "../Parser.mjs";
/**
 * @public
 */
/**
 * @public
 */
class YAML1Parser extends Parser {
  refractorOpts;
  constructor(options) {
    const {
      fileExtensions = [],
      mediaTypes = YAMLMediaTypes,
      ...rest
    } = options !== null && options !== void 0 ? options : {};
    super({
      ...rest,
      name: 'yaml-1-2',
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
      return detect(file.toString());
    }
    return false;
  }
  async parse(file) {
    const source = file.toString();
    try {
      return await parse(source, {
        sourceMap: this.sourceMap
      });
    } catch (error) {
      throw new ParserError(`Error parsing "${file.uri}"`, {
        cause: error
      });
    }
  }
}
export default YAML1Parser;