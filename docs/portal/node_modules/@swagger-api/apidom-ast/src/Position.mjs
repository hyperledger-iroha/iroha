/* eslint-disable max-classes-per-file */

/**
 * @public
 */

/**
 * @public
 */
export class Point {
  static type = 'point';
  type = Point.type;
  row;
  column;
  char;
  constructor({
    row,
    column,
    char
  }) {
    this.row = row;
    this.column = column;
    this.char = char;
  }
}

/**
 * @public
 */

/**
 * @public
 */
class Position {
  static type = 'position';
  type = Position.type;
  start;
  end;
  constructor({
    start,
    end
  }) {
    this.start = start;
    this.end = end;
  }
}
export default Position;