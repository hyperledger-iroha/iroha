/**
 * @public
 */

/**
 * @public
 */
class Reference {
  uri;
  depth;
  value;
  refSet;
  errors;
  constructor({
    uri,
    depth = 0,
    refSet,
    value
  }) {
    this.uri = uri;
    this.value = value;
    this.depth = depth;
    this.refSet = refSet;
    this.errors = [];
  }
}
export default Reference;