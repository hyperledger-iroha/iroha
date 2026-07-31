import { specificationObj as OpenApi3_1Specification } from '@swagger-api/apidom-ns-openapi-3-0';
import MediaTypeElement from "../../../../elements/MediaType.mjs";
/**
 * @public
 */
export const BaseMediaTypeVisitor = OpenApi3_1Specification.visitors.document.objects.MediaType.$visitor;
/**
 * @public
 */
class MediaTypeVisitor extends BaseMediaTypeVisitor {
  constructor(options) {
    super(options);
    this.element = new MediaTypeElement();
  }
}
export default MediaTypeVisitor;