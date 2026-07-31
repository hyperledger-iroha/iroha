"use strict";

var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
exports.__esModule = true;
exports.readFile = void 0;
var _ramda = require("ramda");
var _File = _interopRequireDefault(require("../File.cjs"));
var plugins = _interopRequireWildcard(require("../util/plugins.cjs"));
var _ResolveError = _interopRequireDefault(require("../errors/ResolveError.cjs"));
var _UnmatchedResolverError = _interopRequireDefault(require("../errors/UnmatchedResolverError.cjs"));
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
const readFile = async (file, options) => {
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
    throw new _ResolveError.default(`Error while reading file "${file.uri}"`, {
      cause: cachedError
    });
  }
  const optsBoundResolvers = options.resolve.resolvers.map(resolver => {
    const clonedResolver = Object.create(resolver);
    return Object.assign(clonedResolver, options.resolve.resolverOpts);
  });
  const resolvers = await plugins.filter('canRead', [file, options], optsBoundResolvers);

  // we couldn't find any resolver for this File
  if ((0, _ramda.isEmpty)(resolvers)) {
    throw new _UnmatchedResolverError.default(file.uri);
  }
  try {
    const {
      result
    } = await plugins.run('read', [file], resolvers);
    const stringifiedData = new _File.default({
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
      error: (_ref = (_error$cause$cause = error == null || (_error$cause = error.cause) == null ? void 0 : _error$cause.cause) != null ? _error$cause$cause : error == null ? void 0 : error.cause) != null ? _ref : error,
      cacheTTL
    });
    throw new _ResolveError.default(`Error while reading file "${file.uri}"`, {
      cause: error
    });
  }
};
exports.readFile = readFile;