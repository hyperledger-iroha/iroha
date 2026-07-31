class EphemeralObject {
  type = 'EphemeralObject';
  content = [];
  reference = undefined;
  constructor(content) {
    this.content = content;
    this.reference = {};
  }
  toReference() {
    return this.reference;
  }
  toObject() {
    return Object.assign(this.reference, Object.fromEntries(this.content));
  }
}
export default EphemeralObject;