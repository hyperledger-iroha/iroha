import { unescape as baseUnescape, URIFragmentIdentifier } from '@swaggerexpert/json-pointer';

/**
 * @public
 * @deprecated
 */
const unescape = referenceToken => {
  return URIFragmentIdentifier.from(baseUnescape(referenceToken));
};
export default unescape;