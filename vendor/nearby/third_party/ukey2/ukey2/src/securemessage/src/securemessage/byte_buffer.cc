/*
 * Copyright 2014 Google Inc. All rights reserved.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#include "src/securemessage/include/securemessage/byte_buffer.h"

#include <climits>
#include <cstring>

#include <iomanip>
#include <sstream>

using std::vector;

namespace securemessage {

ByteBuffer::ByteBuffer() {
  // Do nothing
}

ByteBuffer::~ByteBuffer() { Wipe(); }

ByteBuffer::ByteBuffer(const size_t size) {
  // Fills the data buffer with size number of zeros
  data_.resize(size, 0);
}

void ByteBuffer::SetData(const uint8_t* source_data, const size_t length) {
  // reserve() might cause re-allocation, so we wipe first.
  Wipe();
  data_.clear();

  data_.resize(length);
  for (size_t i = 0; i < length; i++) {
    data_[i] = source_data[i];
  }
}

void ByteBuffer::Append(const size_t num_bytes, const uint8_t byte) {
  // harmless on size_t overflow, but might cause reallocation which might leak
  // data on the heap
  data_.reserve(data_.size() + num_bytes);
  data_.insert(data_.end(), num_bytes, byte);
}

void ByteBuffer::Prepend(const string& str) {
  // might cause reallocation which might leak data on the heap
  data_.insert(data_.begin(), str.begin(), str.end());
}

ByteBuffer ByteBuffer::Concat(const ByteBuffer& a, const ByteBuffer& b) {
  // We assume compiler implements NRVO here.  We avoid initializing
  // return_value via a.Copy() because reserve() may result in another
  // data copy, which we are trying to eliminate in the first place.
  ByteBuffer return_value;

  return_value.data_.reserve(a.size() + b.size());  // harmless on overflow
  return_value.data_.insert(return_value.data_.end(), a.data_.begin(),
                            a.data_.end());
  return_value.data_.insert(return_value.data_.end(), b.data_.begin(),
                            b.data_.end());
  return return_value;
}

// GCC before 4.7 doesn't support delegated constructors, so we can't be as
// elegant as we could otherwise.
ByteBuffer::ByteBuffer(const uint8_t* source_data, const size_t length) {
  SetData(source_data, length);
}

ByteBuffer::ByteBuffer(const char* source_data) {
  SetData(reinterpret_cast<const uint8_t*>(source_data), strlen(source_data));
}

ByteBuffer::ByteBuffer(const char* source_data, const size_t length) {
  SetData(reinterpret_cast<const uint8_t*>(source_data), length);
}

ByteBuffer::ByteBuffer(const string& source_data) {
  SetData(reinterpret_cast<const uint8_t*>(source_data.c_str()),
          source_data.length());
}

size_t ByteBuffer::size() const { return data_.size(); }

std::unique_ptr<ByteBuffer> ByteBuffer::SubArray(const size_t offset,
                                                 const size_t length) const {
  if (length == 0) {
    return std::unique_ptr<ByteBuffer>(new ByteBuffer());
  }

  // Overflow check
  if (length > UINT_MAX - offset) {
    return nullptr;
  }

  // Bounds check
  if (offset + length > size()) {
    return nullptr;
  }

  return std::unique_ptr<ByteBuffer>(
      new ByteBuffer(ImmutableUChar() + offset, length));
}

unsigned char* ByteBuffer::MutableUChar() {
  return reinterpret_cast<unsigned char*>(MutableUInt8());
}

unsigned const char* ByteBuffer::ImmutableUChar() const {
  return reinterpret_cast<unsigned const char*>(data_.data());
}

uint8_t* ByteBuffer::MutableUInt8() { return data_.data(); }

const uint8_t* ByteBuffer::ImmutableUInt8() const {
  return reinterpret_cast<const uint8_t*>(data_.data());
}

string ByteBuffer::String() const {
  return string(reinterpret_cast<const char*>(data_.data()), data_.size());
}

vector<uint8_t> ByteBuffer::Vector() const { return vector<uint8_t>(data_); }

bool ByteBuffer::Equals(const ByteBuffer& other) const {
  if (other.size() != size()) {
    return false;
  }

  // Make sure to always iterate over all elements to defeat timing attacks
  unsigned int result = 0;
  for (size_t i = 0; i < size(); i++) {
    result |= data_[i] ^ other.ImmutableUInt8()[i];
  }

  return (result == 0);
}

bool ByteBuffer::Equals(const string& other) const {
  return Equals(ByteBuffer(other));
}

string ByteBuffer::AsDebugHexString() const {
  if (data_.size() == 0) {
    return string();
  }

  const char hex[] = "0123456789abcdef";
  string result = string("0x");

  for (size_t i = 0; i < data_.size(); i++) {
    result.push_back(hex[0xf & (data_[i] >> 4)]);
    result.push_back(hex[0xf & data_[i]]);
  }

  return result;
}

void ByteBuffer::Clear() {
  Wipe();
  data_.clear();
}

void ByteBuffer::Wipe() { WipeBuffer(data_.data(), data_.size()); }

void ByteBuffer::WipeBuffer(uint8_t* buffer, const size_t length) {
  volatile uint8_t* byte_to_wipe = buffer;
  for (size_t i = 0; i < length; i++) {
    byte_to_wipe[i] = 0;
  }
}

}  // namespace securemessage
