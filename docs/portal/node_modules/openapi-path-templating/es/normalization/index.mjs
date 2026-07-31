import percentEndingNormalizer from "./percent-encoding.mjs";
import caseNormalizer from "./case.mjs";
import pathSegmentNormalizer from "./path-segment.mjs";
/**
 * Implementing https://datatracker.ietf.org/doc/html/rfc3986#section-6.2
 */
const normalize = pathTemplate => {
  const decodedPath = percentEndingNormalizer(pathTemplate);
  const caseNormalizedPath = caseNormalizer(decodedPath);
  return pathSegmentNormalizer(caseNormalizedPath);
};
export { percentEndingNormalizer, caseNormalizer, pathSegmentNormalizer };
export { default as identityNormalizer } from "./identity.mjs";
export default normalize;