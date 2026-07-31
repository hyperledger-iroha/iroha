import { StringElement } from 'minim';

/**
 * @public
 */
class Comment extends StringElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'comment';
  }
}
export default Comment;