import { propOr, omit } from 'ramda';
import { isNotUndefined } from 'ramda-adjunct';
import { createNamespace } from '@swagger-api/apidom-core';
import { parse as parseJSON, detect as detectJSON } from '@swagger-api/apidom-parser-adapter-json';
import openApiNamespace, { SwaggerElement } from '@swagger-api/apidom-ns-openapi-2';
export { default as mediaTypes } from "./media-types.mjs";
/**
 * @public
 */
export const detectionRegExp = /"swagger"\s*:\s*"(?<version_json>2\.0)"/;

/**
 * @public
 */
export const detect = async source => detectionRegExp.test(source) && (await detectJSON(source));

/**
 * @public
 */
export const parse = async (source, options = {}) => {
  const refractorOpts = propOr({}, 'refractorOpts', options);
  const parserOpts = omit(['refractorOpts'], options);
  const parseResultElement = await parseJSON(source, parserOpts);
  const {
    result
  } = parseResultElement;
  if (isNotUndefined(result)) {
    const swaggerElement = SwaggerElement.refract(result, refractorOpts);
    swaggerElement.classes.push('result');
    parseResultElement.replaceResult(swaggerElement);
  }
  return parseResultElement;
};

/**
 * @public
 */
export const namespace = createNamespace(openApiNamespace);