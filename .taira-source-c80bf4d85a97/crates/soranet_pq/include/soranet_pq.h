// Generated soranet_pq FFI header. Update via cbindgen when making API changes.
#ifndef SORANET_PQ_H
#define SORANET_PQ_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SORANET_PQ_SUCCESS 0
#define SORANET_PQ_ERR_INVALID_SUITE -1
#define SORANET_PQ_ERR_NULL_POINTER -2
#define SORANET_PQ_ERR_LENGTH_MISMATCH -3
#define SORANET_PQ_ERR_ENCODING -4
#define SORANET_PQ_ERR_KEYGEN -5
#define SORANET_PQ_ERR_VERIFICATION_FAILED -6

int soranet_mlkem_parameters(uint32_t suite_id,
                             uint32_t *public_key_len_out,
                             uint32_t *secret_key_len_out,
                             uint32_t *ciphertext_len_out,
                             uint32_t *shared_secret_len_out);

int soranet_mlkem_generate_keypair(uint32_t suite_id,
                                   uint8_t *public_key_out,
                                   size_t public_key_len,
                                   uint8_t *secret_key_out,
                                   size_t secret_key_len);

int soranet_mlkem_encapsulate(uint32_t suite_id,
                              const uint8_t *public_key,
                              size_t public_key_len,
                              uint8_t *ciphertext_out,
                              size_t ciphertext_len,
                              uint8_t *shared_secret_out,
                              size_t shared_secret_len);

int soranet_mlkem_decapsulate(uint32_t suite_id,
                              const uint8_t *secret_key,
                              size_t secret_key_len,
                              const uint8_t *ciphertext,
                              size_t ciphertext_len,
                              uint8_t *shared_secret_out,
                              size_t shared_secret_len);

int soranet_mldsa_parameters(uint32_t suite_id,
                             uint32_t *public_key_len_out,
                             uint32_t *secret_key_len_out,
                             uint32_t *signature_len_out);

int soranet_mldsa_generate_keypair(uint32_t suite_id,
                                   uint8_t *public_key_out,
                                   size_t public_key_len,
                                   uint8_t *secret_key_out,
                                   size_t secret_key_len);

int soranet_mldsa_sign(uint32_t suite_id,
                       const uint8_t *secret_key,
                       size_t secret_key_len,
                       const uint8_t *message,
                       size_t message_len,
                       uint8_t *signature_out,
                       size_t signature_len);

int soranet_mldsa_verify(uint32_t suite_id,
                         const uint8_t *public_key,
                         size_t public_key_len,
                         const uint8_t *message,
                         size_t message_len,
                         const uint8_t *signature,
                         size_t signature_len);

#ifdef __cplusplus
}
#endif

#endif // SORANET_PQ_H
