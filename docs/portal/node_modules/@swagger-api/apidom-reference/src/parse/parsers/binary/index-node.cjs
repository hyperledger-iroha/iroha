"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.default = void 0;
var _buffer = require("#buffer");
var _apidomCore = require("@swagger-api/apidom-core");
var _ParserError = _interopRequireDefault(require("../../../errors/ParserError.cjs"));
var _Parser = _interopRequireDefault(require("../Parser.cjs"));
// eslint-disable-line import/order

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
    let base64String;
    try {
      base64String = _buffer.Buffer.from(file.data).toString('base64');
    } catch {
      base64String = _buffer.Buffer.from(file.toString()).toString('base64');
    }
    try {
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