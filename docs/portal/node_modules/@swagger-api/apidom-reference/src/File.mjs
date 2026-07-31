import { type } from 'ramda';
import { isString } from 'ramda-adjunct';
import * as url from "./util/url.mjs";
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
    if (isString(this.uri)) {
      return url.getExtension(this.uri);
    }
    return '';
  }
  toString() {
    if (typeof this.data === 'string') {
      return this.data;
    }
    if (this.data instanceof ArrayBuffer || ['ArrayBuffer'].includes(type(this.data)) || ArrayBuffer.isView(this.data)) {
      const textDecoder = new TextDecoder('utf-8');
      return textDecoder.decode(this.data);
    }
    return String(this.data);
  }
}
export default File;