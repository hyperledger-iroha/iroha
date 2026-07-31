class EphemeralArray {
  type = 'EphemeralArray';
  content = [];
  reference = undefined;
  constructor(content) {
    this.content = content;
    this.reference = [];
  }
  toReference() {
    return this.reference;
  }
  toArray() {
    this.reference.push(...this.content);
    return this.reference;
  }
}
export default EphemeralArray;