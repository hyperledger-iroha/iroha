import { ParseResultElement, StringElement } from '@swagger-api/apidom-core';
import ParserError from "../../../errors/ParserError.mjs";
import Parser from "../Parser.mjs";
/**
 * Everything that is not recognized by other parsers will be considered by this parser
 * as a binary data and will be encoded to Base64 format.
 * @public
 */
/**
 * @public
 */
class BinaryParser extends Parser {
  constructor(options) {
    super({
      ...(options !== null && options !== void 0 ? options : {}),
      name: 'binary'
    });
  }
  canParse(file) {
    return this.fileExtensions.length === 0 ? true : this.fileExtensions.includes(file.extension);
  }

  // eslint-disable-next-line class-methods-use-this
  parse(file) {
    try {
      /**
       * More information about binary strings and btoa function in following link:
       *   https://developer.mozilla.org/en-US/docs/Web/API/btoa
       *
       * @example
       * ArrayBuffer to base64 conversion:
       *
       * const binaryString = String.fromCharCode.apply(null, file.data);
       * base64String = btoa(binaryString);
       */
      const binaryString = unescape(encodeURIComponent(file.toString()));
      const base64String = btoa(binaryString);
      const parseResultElement = new ParseResultElement();
      if (base64String.length !== 0) {
        const base64StringElement = new StringElement(base64String);
        base64StringElement.classes.push('result');
        parseResultElement.push(base64StringElement);
      }
      return parseResultElement;
    } catch (error) {
      throw new ParserError(`Error parsing "${file.uri}"`, {
        cause: error
      });
    }
  }
}
export default BinaryParser;