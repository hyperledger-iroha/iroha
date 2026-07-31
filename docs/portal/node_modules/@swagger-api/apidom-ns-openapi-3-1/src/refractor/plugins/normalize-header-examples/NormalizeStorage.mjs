import { ArrayElement, ObjectElement, isObjectElement, isArrayElement } from '@swagger-api/apidom-core';
class NormalizeStorage {
  internalStore;
  constructor(storageElement, storageField, storageSubField) {
    this.storageElement = storageElement;
    this.storageField = storageField;
    this.storageSubField = storageSubField;
  }
  get store() {
    if (!this.internalStore) {
      let rootStore = this.storageElement.get(this.storageField);
      if (!isObjectElement(rootStore)) {
        rootStore = new ObjectElement();
        this.storageElement.set(this.storageField, rootStore);
      }
      let store = rootStore.get(this.storageSubField);
      if (!isArrayElement(store)) {
        store = new ArrayElement();
        rootStore.set(this.storageSubField, store);
      }
      this.internalStore = store;
    }
    return this.internalStore;
  }
  append(pointer) {
    if (!this.includes(pointer)) {
      this.store.push(pointer);
    }
  }
  includes(pointer) {
    return this.store.includes(pointer);
  }
}
export default NormalizeStorage;