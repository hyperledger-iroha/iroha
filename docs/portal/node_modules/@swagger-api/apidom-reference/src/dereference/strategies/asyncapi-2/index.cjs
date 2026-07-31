"use strict";

var _interopRequireDefault = require("@babel/runtime-corejs3/helpers/interopRequireDefault").default;
var _interopRequireWildcard = require("@babel/runtime-corejs3/helpers/interopRequireWildcard").default;
exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
var _apidomNsAsyncapi = _interopRequireWildcard(require("@swagger-api/apidom-ns-asyncapi-2"));
var _DereferenceStrategy = _interopRequireDefault(require("../DereferenceStrategy.cjs"));
var _Reference = _interopRequireDefault(require("../../../Reference.cjs"));
var _ReferenceSet = _interopRequireDefault(require("../../../ReferenceSet.cjs"));
var _visitor = _interopRequireDefault(require("./visitor.cjs"));
exports.AsyncAPI2DereferenceVisitor = _visitor.default;
// @ts-ignore
const visitAsync = _apidomCore.visit[Symbol.for('nodejs.util.promisify.custom')];

/**
 * @public
 */

/**
 * @public
 */
class AsyncAPI2DereferenceStrategy extends _DereferenceStrategy.default {
  constructor(options) {
    super({
      ...(options != null ? options : {}),
      name: 'asyncapi-2'
    });
  }
  canDereference(file) {
    var _file$parseResult;
    // assert by media type
    if (file.mediaType !== 'text/plain') {
      return _apidomNsAsyncapi.mediaTypes.includes(file.mediaType);
    }

    // assert by inspecting ApiDOM
    return (0, _apidomNsAsyncapi.isAsyncApi2Element)((_file$parseResult = file.parseResult) == null ? void 0 : _file$parseResult.api);
  }
  async dereference(file, options) {
    var _options$dereference$;
    const namespace = (0, _apidomCore.createNamespace)(_apidomNsAsyncapi.default);
    const immutableRefSet = (_options$dereference$ = options.dereference.refSet) != null ? _options$dereference$ : new _ReferenceSet.default();
    const mutableRefSet = new _ReferenceSet.default();
    let refSet = immutableRefSet;
    let reference;
    if (!immutableRefSet.has(file.uri)) {
      reference = new _Reference.default({
        uri: file.uri,
        value: file.parseResult
      });
      immutableRefSet.add(reference);
    } else {
      // pre-computed refSet was provided as configuration option
      reference = immutableRefSet.find(ref => ref.uri === file.uri);
    }

    /**
     * Clone refSet due the dereferencing process being mutable.
     * We don't want to mutate the original refSet and the references.
     */
    if (options.dereference.immutable) {
      immutableRefSet.refs.map(ref => new _Reference.default({
        ...ref,
        value: (0, _apidomCore.cloneDeep)(ref.value)
      })).forEach(ref => mutableRefSet.add(ref));
      reference = mutableRefSet.find(ref => ref.uri === file.uri);
      refSet = mutableRefSet;
    }
    const visitor = new _visitor.default({
      reference,
      namespace,
      options
    });
    const dereferencedElement = await visitAsync(refSet.rootRef.value, visitor, {
      keyMap: _apidomNsAsyncapi.keyMap,
      nodeTypeGetter: _apidomNsAsyncapi.getNodeType
    });

    /**
     * If immutable option is set, replay refs from the refSet.
     */
    if (options.dereference.immutable) {
      mutableRefSet.refs.filter(ref => ref.uri.startsWith('immutable://')).map(ref => new _Reference.default({
        ...ref,
        uri: ref.uri.replace(/^immutable:\/\//, '')
      })).forEach(ref => immutableRefSet.add(ref));
    }

    /**
     * Release all memory if this refSet was not provided as a configuration option.
     * If provided as configuration option, then provider is responsible for cleanup.
     */
    if (options.dereference.refSet === null) {
      immutableRefSet.clean();
    }
    mutableRefSet.clean();
    return dereferencedElement;
  }
}
var _default = exports.default = AsyncAPI2DereferenceStrategy;