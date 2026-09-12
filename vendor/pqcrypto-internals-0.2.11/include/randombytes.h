#ifndef RANDOMBYTES_H
#define RANDOMBYTES_H
#include <stdint.h>

#ifdef _WIN32
#include <crtdefs.h>
#else
#include <unistd.h>
#endif

#define randombytes PQCRYPTO_RUST_randombytes
int randombytes(uint8_t *buf, size_t n);

#endif
