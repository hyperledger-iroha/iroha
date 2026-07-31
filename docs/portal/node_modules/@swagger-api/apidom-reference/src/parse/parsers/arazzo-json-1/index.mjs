import { pick } from 'ramda';
import { parse, mediaTypes as Arazzo1MediaTypes, detect } from '@swagger-api/apidom-parser-adapter-arazzo-json-1';
import ParserError from "../../../errors/ParserError.mjs";
import Parser from "../Parser.mjs";
/**
 * @public
 */
/**
 * @public
 */
class ArazzoJSON1Parser extends Parser {
  syntacticAnalysis;
  refractorOpts;
  constructor(options) {
    const {
      fileExtensions = [],
      mediaTypes = Arazzo1MediaTypes,
      ...rest
    } = options !== null && options !== void 0 ? options : {};
    super({
      ...rest,
      name: 'arazzo-json-1',
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
      const parserOpts = pick(['sourceMap', 'syntacticAnalysis', 'refractorOpts'], this);
      return await parse(source, parserOpts);
    } catch (error) {
      throw new ParserError(`Error parsing "${file.uri}"`, {
        cause: error
      });
    }
  }
}
export default ArazzoJSON1Parser;