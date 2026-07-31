import { isEmpty } from 'ramda';
import * as url from "../util/url.mjs";
import File from "../File.mjs";
import * as plugins from "../util/plugins.mjs";
import ParseError from "../errors/ParseError.mjs";
import UnmatchedResolverError from "../errors/UnmatchedResolverError.mjs";
import { readFile } from "../resolve/util.mjs";
/**
 * Parses the given file's contents, using the configured parser plugins.
 */
const parseFile = async (file, options) => {
  const optsBoundParsers = options.parse.parsers.map(parser => {
    const clonedParser = Object.create(parser);
    return Object.assign(clonedParser, options.parse.parserOpts);
  });
  const parsers = await plugins.filter('canParse', [file, options], optsBoundParsers);

  // we couldn't find any parser for this File
  if (isEmpty(parsers)) {
    throw new UnmatchedResolverError(file.uri);
  }
  try {
    const {
      plugin,
      result
    } = await plugins.run('parse', [file, options], parsers);

    // empty files handling
    if (!plugin.allowEmpty && result.isEmpty) {
      return Promise.reject(new ParseError(`Error while parsing file "${file.uri}". File is empty.`));
    }
    return result;
  } catch (error) {
    throw new ParseError(`Error while parsing file "${file.uri}"`, {
      cause: error
    });
  }
};

/**
 * Parses a file into ApiDOM.
 */
const parse = async (uri, options) => {
  /**
   * If the path is a filesystem path, then convert it to a URL.
   *
   * NOTE: According to the JSON Reference spec, these should already be URLs,
   * but, in practice, many people use local filesystem paths instead.
   * So we're being generous here and doing the conversion automatically.
   * This is not intended to be a 100% bulletproof solution.
   * If it doesn't work for your use-case, then use a URL instead.
   */
  const file = new File({
    uri: url.sanitize(url.stripHash(uri)),
    mediaType: options.parse.mediaType
  });
  const data = await readFile(file, options);
  return parseFile(new File({
    ...file,
    data
  }), options);
};
export default parse;