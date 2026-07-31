import { propOr, omit } from 'ramda';
import { isNotUndefined } from 'ramda-adjunct';
import { createNamespace } from '@swagger-api/apidom-core';
import { parse as parseYAML, detect as detectYAML } from '@swagger-api/apidom-parser-adapter-yaml-1-2';
import openApiNamespace, { SwaggerElement } from '@swagger-api/apidom-ns-openapi-2';
export { default as mediaTypes } from "./media-types.mjs";
/**
 * @public
 */
export const detectionRegExp = /(?<YAML>^(["']?)swagger\2\s*:\s*(["'])(?<version_yaml>2\.0)\3(?:\s+|$))|(?<JSON>"swagger"\s*:\s*"(?<version_json>2\.0)")/m;

/**
 * @public
 */
export const detect = async source => detectionRegExp.test(source) && (await detectYAML(source));

/**
 * @public
 */
export const parse = async (source, options = {}) => {
  const refractorOpts = propOr({}, 'refractorOpts', options);
  const parserOpts = omit(['refractorOpts'], options);
  const parseResultElement = await parseYAML(source, parserOpts);
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