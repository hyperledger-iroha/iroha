"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _ParserError = _interopRequireDefault(require("../../../errors/ParserError.cjs"));
var _Parser = _interopRequireDefault(require("../Parser.cjs"));
/**
 * Everything that is not recognized by other parsers will be considered by this parser
 * as a binary data and will be encoded to Base64 format.
 * @public
 */

/**
 * @public
 */
class BinaryParser extends _Parser.default {
  constructor(options) {
    super({
      ...(options != null ? options : {}),
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
      const parseResultElement = new _apidomCore.ParseResultElement();
      if (base64String.length !== 0) {
        const base64StringElement = new _apidomCore.StringElement(base64String);
        base64StringElement.classes.push('result');
        parseResultElement.push(base64StringElement);
      }
      return parseResultElement;
    } catch (error) {
      throw new _ParserError.default(`Error parsing "${file.uri}"`, {
        cause: error
      });
    }
  }
}
var _default = exports.default = BinaryParser;