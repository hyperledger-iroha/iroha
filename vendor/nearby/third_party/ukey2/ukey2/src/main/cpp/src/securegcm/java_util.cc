// Copyright 2020 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#include "src/main/cpp/include/securegcm/java_util.h"

#include <cstring>

namespace securegcm {
namespace java_util {

namespace {

// Returns the lower 32-bits of a int64_t |value| as an int32_t.
int32_t Lower32Bits(int64_t value) {
  const uint32_t lower_bits = static_cast<uint32_t>(value & 0xFFFFFFFF);
  int32_t return_value;
  std::memcpy(&return_value, &lower_bits, sizeof(uint32_t));
  return return_value;
}

}  // namespace

int32_t JavaMultiply(int32_t lhs, int32_t rhs) {
  // Multiplication guaranteed to fit in int64_t, range from [2^63, 2^63 - 1].
  // Minimum value is (-2^31)^2 = 2^62.
  const int64_t result = static_cast<int64_t>(lhs) * static_cast<int64_t>(rhs);
  return Lower32Bits(result);
}

int32_t JavaAdd(int32_t lhs, int32_t rhs) {
  const int64_t result = static_cast<int64_t>(lhs) + static_cast<int64_t>(rhs);
  return Lower32Bits(result);
}

int32_t JavaHashCode(const securemessage::ByteBuffer& byte_buffer) {
  const string bytes = byte_buffer.String();
  int32_t hash_code = 1;
  for (const int8_t byte : bytes) {
    int32_t int_value = static_cast<int32_t>(byte);
    // Java relies on the overflow/underflow behaviour of arithmetic operations,
    // which is undefined in C++, so we call our own Java-compatible versions of
    // + and * here.
    hash_code = JavaAdd(JavaMultiply(31, hash_code), int_value);
  }
  return hash_code;
}

}  // namespace java_util
}  // namespace securegcm
