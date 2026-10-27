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

#include <iostream>

#include "src/securemessage/include/securemessage/util.h"

using std::unique_ptr;

namespace securemessage {

unique_ptr<string> Util::MakeUniquePtrString(const void* data, size_t length) {
  return unique_ptr<string>(new string((const char*)data, length));
}

void Util::LogError(const string& error_message) {
  std::cerr << "[ERROR] " << error_message << std::endl;
}

void Util::LogErrorAndAbort(const string& error_message) {
  LogError(error_message);
  abort();
}

Util::~Util() {}

}  // namespace securemessage
