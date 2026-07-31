import { mediaTypes, isOpenApi3_1Element } from '@swagger-api/apidom-ns-openapi-3-1';
import BundleStrategy from "../BundleStrategy.mjs";
/**
 * @public
 */
/**
 * @public
 */
class OpenAPI3_1BundleStrategy extends BundleStrategy {
  constructor(options) {
    super({
      ...(options !== null && options !== void 0 ? options : {}),
      name: 'openapi-3-1'
    });
  }
  canBundle(file) {
    var _file$parseResult;
    // assert by media type
    if (file.mediaType !== 'text/plain') {
      return mediaTypes.includes(file.mediaType);
    }

    // assert by inspecting ApiDOM
    return isOpenApi3_1Element((_file$parseResult = file.parseResult) === null || _file$parseResult === void 0 ? void 0 : _file$parseResult.result);
  }
  async bundle(file) {
    return file.parseResult;
  }
}
export default OpenAPI3_1BundleStrategy;