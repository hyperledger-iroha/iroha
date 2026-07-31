import { isEmpty } from 'ramda';
import File from "../File.mjs";
import * as plugins from "../util/plugins.mjs";
import ResolveError from "../errors/ResolveError.mjs";
import UnmatchedResolverError from "../errors/UnmatchedResolverError.mjs";
const CACHE_NAME = 'apidom-file-cache';
const getCacheFileResult = async ({
  cacheKey,
  cacheTTL
}) => {
  if (cacheTTL === 0) {
    return {
      cachedResult: null,
      cachedError: null
    };
  }
  try {
    const cache = await caches.open(CACHE_NAME);
    const response = await cache.match(cacheKey);
    if (response) {
      const {
        cachedResult,
        cachedError,
        timestamp
      } = await response.json();
      const now = Date.now();

      // Check if the cache is still valid
      if (now - timestamp < cacheTTL) {
        return {
          cachedResult,
          cachedError
        };
      }
      await cache.delete(cacheKey);
    }
  } catch (error) {
    // If parsing cache fails, continue with normal parsing
    console.log('There was an error parsing the response');
    return {
      cachedResult: null,
      cachedError: null
    };
  }
  return {
    cachedResult: null,
    cachedError: null
  };
};
const setCacheFileResult = async ({
  cacheKey,
  result,
  error,
  cacheTTL
}) => {
  if (cacheTTL === 0) {
    return;
  }
  try {
    const cacheData = {
      cachedResult: result,
      cachedError: error,
      timestamp: Date.now()
    };
    const cache = await caches.open(CACHE_NAME);
    const response = new Response(JSON.stringify(cacheData), {
      headers: {
        'Content-Type': 'application/json'
      }
    });
    await cache.put(cacheKey, response);
  } catch (err) {
    // Silently fail if CacheStorage is not available
    console.error('CacheStorage is not available');
  }
};

/**
 * Reads the given file, using the configured resolver plugins.
 */
// eslint-disable-next-line import/prefer-default-export
export const readFile = async (file, options) => {
  const {
    cacheTTL = 0
  } = options.resolve.resolverOpts;
  const cacheKey = `read_${file.uri}`;
  const {
    cachedResult,
    cachedError
  } = await getCacheFileResult({
    cacheKey,
    cacheTTL
  });
  if (cachedResult !== null) {
    return cachedResult;
  }
  if (cachedError !== null) {
    throw new ResolveError(`Error while reading file "${file.uri}"`, {
      cause: cachedError
    });
  }
  const optsBoundResolvers = options.resolve.resolvers.map(resolver => {
    const clonedResolver = Object.create(resolver);
    return Object.assign(clonedResolver, options.resolve.resolverOpts);
  });
  const resolvers = await plugins.filter('canRead', [file, options], optsBoundResolvers);

  // we couldn't find any resolver for this File
  if (isEmpty(resolvers)) {
    throw new UnmatchedResolverError(file.uri);
  }
  try {
    const {
      result
    } = await plugins.run('read', [file], resolvers);
    const stringifiedData = new File({
      ...file,
      data: result
    }).toString();
    await setCacheFileResult({
      cacheKey,
      result: stringifiedData,
      error: null,
      cacheTTL
    });
    return result;
  } catch (error) {
    var _ref, _error$cause$cause, _error$cause;
    await setCacheFileResult({
      cacheKey,
      result: null,
      error: (_ref = (_error$cause$cause = error === null || error === void 0 || (_error$cause = error.cause) === null || _error$cause === void 0 ? void 0 : _error$cause.cause) !== null && _error$cause$cause !== void 0 ? _error$cause$cause : error === null || error === void 0 ? void 0 : error.cause) !== null && _ref !== void 0 ? _ref : error,
      cacheTTL
    });
    throw new ResolveError(`Error while reading file "${file.uri}"`, {
      cause: error
    });
  }
};