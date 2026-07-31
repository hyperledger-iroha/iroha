import { mediaTypes, ApiDesignSystemsMediaTypes } from '@swagger-api/apidom-ns-api-design-systems';

/**
 * @public
 */
const yamlMediaTypes = new ApiDesignSystemsMediaTypes(...mediaTypes.filterByFormat('generic'), ...mediaTypes.filterByFormat('yaml'));
export default yamlMediaTypes;