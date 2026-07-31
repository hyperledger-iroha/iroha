/* eslint-disable class-methods-use-this */
class Tag {
  static uri = '';
  tag = '';
  constructor() {
    this.tag = this.constructor.uri;
  }

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  test(node) {
    return true;
  }
  resolve(node) {
    return node;
  }
}
/* eslint-enable class-methods-use-this */

export default Tag;