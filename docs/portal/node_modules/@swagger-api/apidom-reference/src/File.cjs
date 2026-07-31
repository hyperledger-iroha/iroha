"use strict";

var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
exports.__esModule = true;
exports.default = void 0;
var _ramda = require("ramda");
var _ramdaAdjunct = require("ramda-adjunct");
var url = _interopRequireWildcard(require("./util/url.cjs"));
/**
 * This class represents a File object with url and data.
 * @public
 */

/**
 * @public
 */
class File {
  uri;
  mediaType;
  data;
  parseResult;
  constructor({
    uri,
    mediaType = 'text/plain',
    data,
    parseResult
  }) {
    this.uri = uri;
    this.mediaType = mediaType;
    this.data = data;
    this.parseResult = parseResult;
  }
  get extension() {
    if ((0, _ramdaAdjunct.isString)(this.uri)) {
      return url.getExtension(this.uri);
    }
    return '';
  }
  toString() {
    if (typeof this.data === 'string') {
      return this.data;
    }
    if (this.data instanceof ArrayBuffer || ['ArrayBuffer'].includes((0, _ramda.type)(this.data)) || ArrayBuffer.isView(this.data)) {
      const textDecoder = new TextDecoder('utf-8');
      return textDecoder.decode(this.data);
    }
    return String(this.data);
  }
}
var _default = exports.default = File;