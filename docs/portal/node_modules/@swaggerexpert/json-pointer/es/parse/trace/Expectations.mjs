class Expectations extends Array {
  toString() {
    return this.map(c => `"${String(c)}"`).join(', ');
  }
}
export default Expectations;