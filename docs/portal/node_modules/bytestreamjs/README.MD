# ByteStream.js

[![License](https://img.shields.io/badge/license-BSD-green.svg?style=flat)](https://github.com/PeculiarVentures/ByteStream.js/blob/master/LICENSE) [![Test](https://github.com/PeculiarVentures/ByteStream.js/actions/workflows/test.yml/badge.svg)](https://github.com/PeculiarVentures/ByteStream.js/actions/workflows/test.yml) [![Known Vulnerabilities](https://snyk.io/test/github/PeculiarVentures/ByteStream.js/badge.svg)](https://snyk.io/test/github/PeculiarVentures/ByteStream.js) [![Coverage Status](https://coveralls.io/repos/github/PeculiarVentures/ByteStream.js/badge.svg?branch=master)](https://coveralls.io/github/PeculiarVentures/ByteStream.js?branch=master)

## ByteStream class

_ByteStream_ class is the major class for all others. It provides many useful function for making search for patterns and data transformation. Optimized for fast as possible data processing.

Method | Description
-------|--------------
_**clear**_|Clear existing stream
_**fromArrayBuffer**_|Initialize "Stream" object from existing "ArrayBuffer"
_**fromUint8Array**_|Initialize "Stream" object from existing "Uint8Array"
_**fromString**_|Initialize "Stream" object from existing string
_**toString**_|Represent "Stream" object content as a string
_**fromHexString**_|Initialize "Stream" object from existing hexdecimal string
_**toHexString**_|Represent "Stream" object content as a hexdecimal string
_**copy**_|Return copy of existing "Stream"
_**slice**_|Return slice of existing "Stream"
_**realloc**_|Change size of existing "Stream"
_**append**_|Append a new "Stream" content to the current "Stream"
_**insert**_|Insert "Stream" content to the current "Stream" at specific position
_**isEqual**_|Check that two "Stream" objects has equal content
_**isEqualView**_|Check that current "Stream" objects has equal content with input "Uint8Array"
_**findPattern**_|Find any byte pattern in "Stream"
_**findFirstIn**_|Find first position of any pattern from input array
_**findAllIn**_|Find all positions of any pattern from input array
_**findAllPatternIn**_|Find all positions of a pattern
_**findFirstNotIn**_|Find first position of data, not included in patterns from input array
_**findAllNotIn**_|Find all positions of data, not included in patterns from input array
_**findFirstSequence**_|Find position of a sequence of any patterns from input array
_**findAllSequences**_|Find all positions of a sequence of any patterns from input array
_**findPairedPatterns**_|Find all paired patterns in the stream
_**findPairedArrays**_|Find all paired patterns in the stream
_**replacePattern**_|Replace one patter with other
_**skipPatterns**_|Skip any pattern from input array
_**skipNotPatterns**_|Skip any pattern not from input array

## SeqStream class

_SeqStream_ class is the aux class for sequential reading/writing data from/to _ByteStream_ underline class.

Method | Description
-------|--------------
_**resetPosition**_|Reset current position of the "SeqStream"
_**findPattern**_|Find any byte pattern in "ByteStream"
_**findFirstIn**_|Find first position of any pattern from input array
_**findAllIn**_|Find all positions of any pattern from input array
_**findFirstNotIn**_|Find first position of data, not included in patterns from input array
_**findAllNotIn**_|Find all positions of data, not included in patterns from input array
_**findFirstSequence**_|Find position of a sequence of any patterns from input array
_**findAllSequences**_|Find position of a sequence of any patterns from input array
_**findPairedPatterns**_|Find all paired patterns in the stream
_**findPairedArrays**_|Find all paired patterns in the stream
_**replacePattern**_|Replace one patter with other
_**skipPatterns**_|Skip of any pattern from input array
_**skipNotPatterns**_|Skip of any pattern from input array
_**append**_|Append a new "Stream" content to the current "Stream"
_**appendView**_|Append a "view" content to the current "Stream"
_**appendChar**_|Append a new char to the current "Stream"
_**getBlock**_|Get a block of data
_**getUint32**_|Get 4-byte unsigned integer value

## BitStream class

Main purpose of the _BitStream_ is manipulating of each bit inside any _ByteStream_ data.

Method | Description
-------|--------------
_**clear**_|Clear existing stream
_**fromByteStream**_|Initialize "BitStream" by data from existing "ByteStream"
_**fromArrayBuffer**_|Initialize "BitStream" object from existing "ArrayBuffer"
_**fromUint8Array**_|Initialize "BitStream" object from existing "Uint8Array"
_**fromString**_|Initialize "BitStream" object from existing bit string
_**toString**_|Represent "BitStream" object content as a string
_**shiftRight**_|Shift entire "BitStream" value right to number of bits
_**shiftLeft**_|Shift entire "BitStream" value left to number of bits
_**slice**_|Return slice of existing "BitStream"
_**copy**_|Return copy of existing "BitStream"
_**shrink**_|Shrink unnecessary bytes in current stream accordingly to "bitsCount" value
_**reverseBytes**_|Reverse bits order in each byte in the stream
_**reverseValue**_|Reverse all bits in entire "BitStream"
_**getNumberValue**_|Trying to represent entire "BitStream" as an unsigned integer
_**findPattern**_|Find any bit pattern in "BitStream"
_**findFirstIn**_|Find first position of any pattern from input array
_**findAllIn**_|Find all positions of any pattern from input array
_**findAllPatternIn**_|Find all positions of a pattern
_**findFirstNotIn**_|Find first position of data, not included in patterns from input array
_**findAllNotIn**_|Find all positions of data, not included in patterns from input array
_**findFirstSequence**_|Find position of a sequence of any patterns from input array
_**findAllSequences**_|Find position of a sequence of any patterns from input array
_**findPairedPatterns**_|Find all paired patterns in the stream
_**findPairedArrays**_|Find all paired patterns in the stream
_**replacePattern**_|Replace one pattern with other
_**skipPatterns**_|Skip any pattern from input array
_**skipNotPatterns**_|Skip any pattern not from input array
_**append**_|Append a new "BitStream" content to the current "BitStream"

## SeqBitStream class

_SeqBitStream_ class is the aux class for sequential reading/writing data from/to _BitStream_ underline class.

Method | Description
-------|--------------
_**getBits**_|Get next "length" bits from the stream
_**getBitsString**_|Get string representation for the next "length" bits from the stream
_**getBitsReversedValue**_|Get number value representation of the next "length" bits from the stream, preliminary reversed
_**toString**_|Represent remaining bits in "BitStream" as a string

## parseByteMap functionality

The `parseByteMap` function is intended to parse and check byte streams with determinated structure.

Example of _map_ used as a kind of _template_. Exactly this _map_ is using for parsing PDF _xref_ table:
```javascript
let map = [
	{
		type: "string",
		name: "type",
		minlength: 1,
		maxlength: 1,
		func: function(array){
			let result = {
				status: (-1),
				length: 1
			};

			switch(array[0])
			{
				case 0x6E: // "n"
					result.value = "n";
					break;
				case 0x66: // "f"
					result.value = "f";
					break;
				default:
					return result;
			}

			result.status = 1;

			return result;
		}
	},
	{
		type: "check",
		minlength: 1,
		maxlength: 2,
		func: function(array){
			let position = (-1);

			if(array[0] == 0x0A)
				position = 1;
			if(array[1] == 0x0A)
				position = 2;

			return {
				status: (position > 0) ? 1 : (-1),
				length: position
			};
		}
	}
];
```

## License

*Copyright (c) 2016-2022, [**Peculiar Ventures**](http://peculiarventures.com/)*
*All rights reserved.*

*Author 2016-2018 [**Yury Strozhevsky**](http://www.strozhevsky.com/).*

Redistribution and use in source and binary forms, with or without modification,
are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice,
   this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its contributors
   may be used to endorse or promote products derived from this software without
   specific prior written permission.

*THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED.
IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT,
INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY
OF SUCH DAMAGE.*
