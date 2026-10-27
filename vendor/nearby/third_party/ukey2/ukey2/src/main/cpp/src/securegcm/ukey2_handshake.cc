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

#include "src/main/cpp/include/securegcm/ukey2_handshake.h"

#include <sstream>

#include "src/main/cpp/include/securegcm/d2d_crypto_ops.h"
#include "src/securemessage/include/securemessage/public_key_proto_util.h"

namespace securegcm {

using securemessage::ByteBuffer;
using securemessage::CryptoOps;
using securemessage::GenericPublicKey;
using securemessage::PublicKeyProtoUtil;

namespace {

// Salt value used to derive client and server keys for next protocol.
const char kUkey2HkdfSalt[] = "UKEY2 v1 next";

// Salt value used to derive verification string.
const char kUkey2VerificationStringSalt[] = "UKEY2 v1 auth";

// Maximum version of the handshake supported by this class.
const uint32_t kVersion = 1;

// Random nonce is fixed at 32 bytes (as per go/ukey2).
const uint32_t kNonceLengthInBytes = 32;

// Currently, we only support one next protocol.
const char kNextProtocol[] = "AES_256_CBC-HMAC_SHA256";

// Creates the appropriate KeyPair for |cipher|.
std::unique_ptr<CryptoOps::KeyPair> GenerateKeyPair(
    UKey2Handshake::HandshakeCipher cipher) {
  switch (cipher) {
    case UKey2Handshake::HandshakeCipher::P256_SHA512:
      return CryptoOps::GenerateEcP256KeyPair();
    default:
      return nullptr;
  }
}

// Parses a CryptoOps::PublicKey from a serialized GenericPublicKey.
std::unique_ptr<securemessage::CryptoOps::PublicKey> ParsePublicKey(
    const string& serialized_generic_public_key) {
  GenericPublicKey generic_public_key;
  if (!generic_public_key.ParseFromString(serialized_generic_public_key)) {
    return nullptr;
  }
  return PublicKeyProtoUtil::ParsePublicKey(generic_public_key);
}

}  // namespace

// static.
std::unique_ptr<UKey2Handshake> UKey2Handshake::ForInitiator(
    HandshakeCipher cipher) {
  return std::unique_ptr<UKey2Handshake>(
      new UKey2Handshake(InternalState::CLIENT_START, cipher));
}

// static.
std::unique_ptr<UKey2Handshake> UKey2Handshake::ForResponder(
    HandshakeCipher cipher) {
  return std::unique_ptr<UKey2Handshake>(
      new UKey2Handshake(InternalState::SERVER_START, cipher));
}

UKey2Handshake::UKey2Handshake(InternalState state, HandshakeCipher cipher)
    : handshake_state_(state),
      handshake_cipher_(cipher),
      handshake_role_(state == InternalState::CLIENT_START
                          ? HandshakeRole::CLIENT
                          : HandshakeRole::SERVER),
      our_key_pair_(GenerateKeyPair(cipher)) {}

UKey2Handshake::State UKey2Handshake::GetHandshakeState() const {
  switch (handshake_state_) {
    case InternalState::CLIENT_START:
    case InternalState::CLIENT_WAITING_FOR_SERVER_INIT:
    case InternalState::CLIENT_AFTER_SERVER_INIT:
    case InternalState::SERVER_START:
    case InternalState::SERVER_AFTER_CLIENT_INIT:
    case InternalState::SERVER_WAITING_FOR_CLIENT_FINISHED:
      // Fallthrough intended -- these are all in-progress states.
      return State::kInProgress;
    case InternalState::HANDSHAKE_VERIFICATION_NEEDED:
      return State::kVerificationNeeded;
    case InternalState::HANDSHAKE_VERIFICATION_IN_PROGRESS:
      return State::kVerificationInProgress;
    case InternalState::HANDSHAKE_FINISHED:
      return State::kFinished;
    case InternalState::HANDSHAKE_ALREADY_USED:
      return State::kAlreadyUsed;
    case InternalState::HANDSHAKE_ERROR:
      return State::kError;
    default:
      // Unreachable.
      return State::kError;
  }
}

const string& UKey2Handshake::GetLastError() const {
  return last_error_;
}

std::unique_ptr<string> UKey2Handshake::GetNextHandshakeMessage() {
  switch (handshake_state_) {
    case InternalState::CLIENT_START: {
      std::unique_ptr<string> client_init = MakeClientInitUkey2Message();
      if (!client_init) {
        // |last_error_| is already set.
        return nullptr;
      }

      wrapped_client_init_ = *client_init;
      handshake_state_ = InternalState::CLIENT_WAITING_FOR_SERVER_INIT;
      return client_init;
    }

    case InternalState::SERVER_AFTER_CLIENT_INIT: {
      std::unique_ptr<string> server_init = MakeServerInitUkey2Message();
      if (!server_init) {
        // |last_error_| is already set.
        return nullptr;
      }

      wrapped_server_init_ = *server_init;
      handshake_state_ = InternalState::SERVER_WAITING_FOR_CLIENT_FINISHED;
      return server_init;
    }

    case InternalState::CLIENT_AFTER_SERVER_INIT: {
      // Make sure we have a message 3 for the chosen cipher.
      if (raw_message3_map_.count(handshake_cipher_) == 0) {
        std::ostringstream stream;
        stream << "Client state is CLIENT_AFTER_SERVER_INIT, and cipher is "
               << static_cast<int>(handshake_cipher_)
               << ", but no corresponding raw "
               << "[Client Finished] message has been generated.";
        SetError(stream.str());
        return nullptr;
      }
      handshake_state_ = InternalState::HANDSHAKE_VERIFICATION_NEEDED;
      return std::unique_ptr<string>(
          new string(raw_message3_map_[handshake_cipher_]));
    }

    default: {
      std::ostringstream stream;
      stream << "Cannot get next message in state "
             << static_cast<int>(handshake_state_);
      SetError(stream.str());
      return nullptr;
    }
  }
}

UKey2Handshake::ParseResult
UKey2Handshake::ParseHandshakeMessage(const string& handshake_message) {
  switch (handshake_state_) {
    case InternalState::SERVER_START:
      return ParseClientInitUkey2Message(handshake_message);
    case InternalState::CLIENT_WAITING_FOR_SERVER_INIT:
      return ParseServerInitUkey2Message(handshake_message);
    case InternalState::SERVER_WAITING_FOR_CLIENT_FINISHED:
      return ParseClientFinishUkey2Message(handshake_message);
    default:
      std::ostringstream stream;
      stream << "Cannot parse message in state "
             << static_cast<int>(handshake_state_);
      SetError(stream.str());
      return {false, nullptr};
  }
}

std::unique_ptr<string> UKey2Handshake::GetVerificationString(int byte_length) {
  if (byte_length < 1 || byte_length > 32) {
    SetError("Minimum length is 1 byte, max is 32 bytes.");
    return nullptr;
  }

  if (handshake_state_ != InternalState::HANDSHAKE_VERIFICATION_NEEDED) {
    std::ostringstream stream;
    stream << "Unexpected state: " << static_cast<int>(handshake_state_);
    SetError(stream.str());
    return nullptr;
  }

  if (!our_key_pair_ || !our_key_pair_->private_key || !their_public_key_) {
    SetError("One of our private key or their public key is null.");
    return nullptr;
  }

  switch (handshake_cipher_) {
    case HandshakeCipher::P256_SHA512:
      derived_secret_key_ = CryptoOps::KeyAgreementSha256(
          *(our_key_pair_->private_key), *their_public_key_);
      break;
    default:
      // Unreachable.
      return nullptr;
  }

  if (!derived_secret_key_) {
    SetError("Failed to derive shared secret key.");
    return nullptr;
  }

  std::unique_ptr<string> auth_string = CryptoOps::Hkdf(
      derived_secret_key_->data().String(),
      string(kUkey2VerificationStringSalt, sizeof(kUkey2VerificationStringSalt)),
      wrapped_client_init_ + wrapped_server_init_);

  handshake_state_ = InternalState::HANDSHAKE_VERIFICATION_IN_PROGRESS;
  return auth_string;
}

bool UKey2Handshake::VerifyHandshake() {
  if (handshake_state_ != InternalState::HANDSHAKE_VERIFICATION_IN_PROGRESS) {
    std::ostringstream stream;
    stream << "Unexpected state: " << static_cast<int>(handshake_state_);
    SetError(stream.str());
    return false;
  }

  handshake_state_ = InternalState::HANDSHAKE_FINISHED;
  return true;
}

std::unique_ptr<D2DConnectionContextV1> UKey2Handshake::ToConnectionContext() {
  if (InternalState::HANDSHAKE_FINISHED != handshake_state_) {
    std::ostringstream stream;
    stream << "ToConnectionContext can only be called when handshake is "
           << "completed, but current state is "
           << static_cast<int>(handshake_state_);
    SetError(stream.str());
    return nullptr;
  }

  if (!derived_secret_key_) {
    SetError("Derived key is null.");
    return nullptr;
  }

  string info = wrapped_client_init_ + wrapped_server_init_;
  std::unique_ptr<string> master_key_data = CryptoOps::Hkdf(
      derived_secret_key_->data().String(), kUkey2HkdfSalt, info);

  if (!master_key_data) {
    SetError("Failed to create master key.");
    return nullptr;
  }

  // Derive separate encode keys for both client and server.
  CryptoOps::SecretKey master_key(*master_key_data, CryptoOps::AES_256_KEY);
  std::unique_ptr<CryptoOps::SecretKey> client_key =
      D2DCryptoOps::DeriveNewKeyForPurpose(master_key, "client");
  std::unique_ptr<CryptoOps::SecretKey> server_key =
      D2DCryptoOps::DeriveNewKeyForPurpose(master_key, "server");
  if (!client_key || !server_key) {
    SetError("Failed to derive client or server key.");
    return nullptr;
  }

  handshake_state_ = InternalState::HANDSHAKE_ALREADY_USED;

  return std::unique_ptr<D2DConnectionContextV1>(new D2DConnectionContextV1(
      handshake_role_ == HandshakeRole::CLIENT ? *client_key : *server_key,
      handshake_role_ == HandshakeRole::CLIENT ? *server_key : *client_key,
      0 /* initial encode sequence number */,
      0 /* initial decode sequence number */));
}

UKey2Handshake::ParseResult UKey2Handshake::ParseClientInitUkey2Message(
    const string& handshake_message) {
  // Deserialize the protobuf.
  Ukey2Message message;
  if (!message.ParseFromString(handshake_message)) {
    return CreateFailedResultWithAlert(Ukey2Alert::BAD_MESSAGE,
                                       "Can't parse message 1.");
  }

  // Verify that message_type == CLIENT_INIT.
  if (!message.has_message_type() ||
      message.message_type() != Ukey2Message::CLIENT_INIT) {
    return CreateFailedResultWithAlert(
        Ukey2Alert::BAD_MESSAGE,
        "Expected, but did not find ClientInit message type.");
  }

  // Derserialize message_data as a ClientInit message.
  if (!message.has_message_data()) {
    return CreateFailedResultWithAlert(
        Ukey2Alert::BAD_MESSAGE_DATA,
        "Expected message data, but did not find it.");
  }

  Ukey2ClientInit client_init;
  if (!client_init.ParseFromString(message.message_data())) {
    return CreateFailedResultWithAlert(
        Ukey2Alert::BAD_MESSAGE_DATA,
        "Can't parse message data into ClientInit.");
  }

  // Check that version == VERSION.
  if (!client_init.has_version()) {
    return CreateFailedResultWithAlert(Ukey2Alert::BAD_VERSION,
                                       "ClientInit missing version.");
  }
  if (client_init.version() != kVersion) {
    return CreateFailedResultWithAlert(Ukey2Alert::BAD_VERSION,
                                       "ClientInit version mismatch.");
  }

  // Check that random is exactly kNonceLengthInBytes.
  if (!client_init.has_random()) {
    return CreateFailedResultWithAlert(Ukey2Alert::BAD_RANDOM,
                                       "ClientInit missing random.");
  }
  if (client_init.random().length() != kNonceLengthInBytes) {
    return CreateFailedResultWithAlert(
        Ukey2Alert::BAD_RANDOM, "ClientInit has incorrect nonce length.");
  }

  // Check to see if any of the handshake_cipher in handshake_cipher_commitment
  // are acceptable. Servers should select the first ahdnshake_cipher that it
  // finds acceptable to support clients signalling deprecated but supported
  // HandshakeCiphers. If no handshake_cipher is acceptable (or there are no
  // HandshakeCiphers in the message), the server sends a BAD_HANDSHAKE_CIPHER
  // alert message.
  if (client_init.cipher_commitments_size() == 0) {
    return CreateFailedResultWithAlert(
        Ukey2Alert::BAD_HANDSHAKE_CIPHER,
        "ClientInit is missing cipher commitments.");
  }

  for (const Ukey2ClientInit::CipherCommitment& commitment :
       client_init.cipher_commitments()) {
    if (!commitment.has_handshake_cipher() || !commitment.has_commitment() ||
        commitment.commitment().empty()) {
      return CreateFailedResultWithAlert(
          Ukey2Alert::BAD_HANDSHAKE_CIPHER,
          "ClientInit has improperly formatted cipher commitment.");
    }

    // TODO(aczeskis): for now we only support one cipher, eventually support
    // more.
    if (commitment.handshake_cipher() == static_cast<int>(handshake_cipher_)) {
      peer_commitment_ = commitment.commitment();
    }
  }

  if (peer_commitment_.empty()) {
    return CreateFailedResultWithAlert(Ukey2Alert::BAD_HANDSHAKE_CIPHER,
                                       "No acceptable commitments found");
  }

  // Checks that next_protocol contains a protocol that the server supports. We
  // currently only support one protocol.
  if (!client_init.has_next_protocol() ||
      client_init.next_protocol() != kNextProtocol) {
    return CreateFailedResultWithAlert(Ukey2Alert::BAD_NEXT_PROTOCOL,
                                       "Incorrect next protocol.");
  }

  // Store raw message for AUTH_STRING computation.
  wrapped_client_init_ = handshake_message;
  handshake_state_ = InternalState::SERVER_AFTER_CLIENT_INIT;
  return CreateSuccessResult();
}

UKey2Handshake::ParseResult UKey2Handshake::ParseServerInitUkey2Message(
    const string& handshake_message) {
  // Deserialize the protobuf.
  Ukey2Message message;
  if (!message.ParseFromString(handshake_message)) {
    return CreateFailedResultWithAlert(Ukey2Alert::BAD_MESSAGE,
                                       "Can't parse message 2.");
  }

  // Verify that message_type == SERVER_INIT.
  if (!message.has_message_type() ||
      message.message_type() != Ukey2Message::SERVER_INIT) {
    return CreateFailedResultWithAlert(
        Ukey2Alert::BAD_MESSAGE,
        "Expected, but did not find SERVER_INIT message type.");
  }

  // Derserialize message_data as a ServerInit message.
  if (!message.has_message_data()) {
    return CreateFailedResultWithAlert(
        Ukey2Alert::BAD_MESSAGE_DATA,
        "Expected message data, but did not find it.");
  }

  Ukey2ServerInit server_init;
  if (!server_init.ParseFromString(message.message_data())) {
    return CreateFailedResultWithAlert(
        Ukey2Alert::BAD_MESSAGE_DATA,
        "Can't parse message data into ServerInit.");
  }

  // Check that version == VERSION.
  if (!server_init.has_version()) {
    return CreateFailedResultWithAlert(Ukey2Alert::BAD_VERSION,
                                       "ServerInit missing version.");
  }
  if (server_init.version() != kVersion) {
    return CreateFailedResultWithAlert(Ukey2Alert::BAD_VERSION,
                                       "ServerInit version mismatch.");
  }

  // Check that random is exactly kNonceLengthInBytes.
  if (!server_init.has_random()) {
    return CreateFailedResultWithAlert(Ukey2Alert::BAD_RANDOM,
                                       "ServerInit missing random.");
  }
  if (server_init.random().length() != kNonceLengthInBytes) {
    return CreateFailedResultWithAlert(
        Ukey2Alert::BAD_RANDOM, "ServerInit has incorrect nonce length.");
  }

  // Check that the handshake_cipher matches a handshake cipher that was sent in
  // ClientInit::cipher_commitments().
  if (!server_init.has_handshake_cipher()) {
    return CreateFailedResultWithAlert(Ukey2Alert::BAD_HANDSHAKE_CIPHER,
                                       "No handshake cipher found.");
  }

  Ukey2HandshakeCipher cipher = server_init.handshake_cipher();
  HandshakeCipher server_cipher;
  switch (static_cast<HandshakeCipher>(cipher)) {
    case HandshakeCipher::P256_SHA512:
      server_cipher = static_cast<HandshakeCipher>(cipher);
      break;
    default:
      return CreateFailedResultWithAlert(Ukey2Alert::BAD_HANDSHAKE_CIPHER,
                                         "No acceptable handshake found.");
  }

  // Check that public_key parses into a correct public key structure.
  if (!server_init.has_public_key()) {
    return CreateFailedResultWithAlert(Ukey2Alert::BAD_PUBLIC_KEY,
                                       "No public key found in ServerInit.");
  }

  their_public_key_ = ParsePublicKey(server_init.public_key());
  if (!their_public_key_) {
    return CreateFailedResultWithAlert(Ukey2Alert::BAD_PUBLIC_KEY,
                                       "Failed to parse public key.");
  }

  // Store raw message for AUTH_STRING computation.
  wrapped_server_init_ = handshake_message;
  handshake_state_ = InternalState::CLIENT_AFTER_SERVER_INIT;
  return CreateSuccessResult();
}

UKey2Handshake::ParseResult UKey2Handshake::ParseClientFinishUkey2Message(
    const string& handshake_message) {
  // Deserialize the protobuf.
  Ukey2Message message;
  if (!message.ParseFromString(handshake_message)) {
    return CreateFailedResultWithoutAlert("Can't parse message 3.");
  }

  // Verify that message_type == CLIENT_FINISH.
  if (!message.has_message_type() ||
      message.message_type() != Ukey2Message::CLIENT_FINISH) {
    return CreateFailedResultWithoutAlert(
        "Expected, but did not find CLIENT_FINISH message type.");
  }

  // Verify that the hash of the CLientFinished message matches the expected
  // commitment from ClientInit.
  if (!VerifyCommitment(handshake_message)) {
    return CreateFailedResultWithoutAlert(last_error_);
  }

  // Deserialize message_data as a ClientFinished message.
  if (!message.has_message_data()) {
    return CreateFailedResultWithoutAlert(
        "Expected message data, but didn't find it.");
  }

  Ukey2ClientFinished client_finished;
  if (!client_finished.ParseFromString(message.message_data())) {
    return CreateFailedResultWithoutAlert("Failed to parse ClientFinished.");
  }

  // Check that public_key parses into a correct public key structure.
  if (!client_finished.has_public_key()) {
    return CreateFailedResultWithoutAlert(
        "No public key found in ClientFinished.");
  }

  their_public_key_ = ParsePublicKey(client_finished.public_key());
  if (!their_public_key_) {
    return CreateFailedResultWithoutAlert("Failed to parse public key.");
  }

  handshake_state_ = InternalState::HANDSHAKE_VERIFICATION_NEEDED;
  return CreateSuccessResult();
}

UKey2Handshake::ParseResult UKey2Handshake::CreateFailedResultWithAlert(
    Ukey2Alert::AlertType alert_type, const string& error_message) {
  if (!Ukey2Alert_AlertType_IsValid(alert_type)) {
    std::ostringstream stream;
    stream << "Unknown alert type: " << static_cast<int>(alert_type);
    SetError(stream.str());
    return {false, nullptr};
  }

  Ukey2Alert alert;
  alert.set_type(alert_type);
  if (!error_message.empty()) {
    alert.set_error_message(error_message);
  }

  std::unique_ptr<string> alert_message =
      MakeUkey2Message(Ukey2Message::ALERT, alert.SerializeAsString());

  SetError(error_message);
  ParseResult result{false, std::move(alert_message)};
  return result;
}

UKey2Handshake::ParseResult
UKey2Handshake::CreateFailedResultWithoutAlert(const string& error_message) {
  SetError(error_message);
  return {false, nullptr};
}

UKey2Handshake::ParseResult UKey2Handshake::CreateSuccessResult() {
  return {true, nullptr};
}

bool UKey2Handshake::VerifyCommitment(const string& handshake_message) {
  std::unique_ptr<ByteBuffer> actual_client_finish_hash;
  switch (handshake_cipher_) {
    case HandshakeCipher::P256_SHA512:
      actual_client_finish_hash =
          CryptoOps::Sha512(ByteBuffer(handshake_message));
      break;
    default:
      // Unreachable.
      return false;
  }

  if (!actual_client_finish_hash) {
    SetError("Failed to hash ClientFinish message.");
    return false;
  }

  // Note: Equals() is a time constant comparison operation.
  if (!actual_client_finish_hash->Equals(peer_commitment_)) {
    SetError("Failed to verify commitment.");
    return false;
  }

  return true;
}

std::unique_ptr<Ukey2ClientInit::CipherCommitment>
UKey2Handshake::GenerateP256Sha512Commitment() {
  // Generate the corresponding ClientFinished message if it's not done yet.
  if (raw_message3_map_.count(HandshakeCipher::P256_SHA512) == 0) {
    if (!our_key_pair_ || !our_key_pair_->public_key) {
      SetError("Invalid public key.");
      return nullptr;
    }

    std::unique_ptr<GenericPublicKey> generic_public_key =
        PublicKeyProtoUtil::EncodePublicKey(*(our_key_pair_->public_key));
    if (!generic_public_key) {
      SetError("Failed to encode generic public key.");
      return nullptr;
    }

    Ukey2ClientFinished client_finished;
    client_finished.set_public_key(generic_public_key->SerializeAsString());
    std::unique_ptr<string> serialized_ukey2_message = MakeUkey2Message(
        Ukey2Message::CLIENT_FINISH, client_finished.SerializeAsString());
    if (!serialized_ukey2_message) {
      SetError("Failed to serialized Ukey2Message.");
      return nullptr;
    }

    raw_message3_map_[HandshakeCipher::P256_SHA512] = *serialized_ukey2_message;
  }

  // Create the SHA512 commitment from raw message 3.
  std::unique_ptr<ByteBuffer> commitment = CryptoOps::Sha512(
      ByteBuffer(raw_message3_map_[HandshakeCipher::P256_SHA512]));
  if (!commitment) {
    SetError("Failed to hash message for commitment.");
    return nullptr;
  }

  // Wrap the commitment in a proto.
  std::unique_ptr<Ukey2ClientInit::CipherCommitment>
      handshake_cipher_commitment(new Ukey2ClientInit::CipherCommitment());
  handshake_cipher_commitment->set_handshake_cipher(P256_SHA512);
  handshake_cipher_commitment->set_commitment(commitment->String());

  return handshake_cipher_commitment;
}

std::unique_ptr<string> UKey2Handshake::MakeClientInitUkey2Message() {
  std::unique_ptr<ByteBuffer> nonce =
      CryptoOps::SecureRandom(kNonceLengthInBytes);
  if (!nonce) {
    SetError("Failed to generate nonce.");
    return nullptr;
  }

  Ukey2ClientInit client_init;
  client_init.set_version(kVersion);
  client_init.set_random(nonce->String());
  client_init.set_next_protocol(kNextProtocol);

  // At the moment, we only support one cipher.
  std::unique_ptr<Ukey2ClientInit::CipherCommitment>
      handshake_cipher_commitment = GenerateP256Sha512Commitment();
  if (!handshake_cipher_commitment) {
    // |last_error_| already set.
    return nullptr;
  }
  *(client_init.add_cipher_commitments()) = *handshake_cipher_commitment;

  return MakeUkey2Message(Ukey2Message::CLIENT_INIT,
                          client_init.SerializeAsString());
}

std::unique_ptr<string> UKey2Handshake::MakeServerInitUkey2Message() {
  std::unique_ptr<ByteBuffer> nonce =
      CryptoOps::SecureRandom(kNonceLengthInBytes);
  if (!nonce) {
    SetError("Failed to generate nonce.");
    return nullptr;
  }

  if (!our_key_pair_ || !our_key_pair_->public_key) {
    SetError("Invalid key pair.");
    return nullptr;
  }

  std::unique_ptr<GenericPublicKey> public_key =
      PublicKeyProtoUtil::EncodePublicKey(*(our_key_pair_->public_key));
  if (!public_key) {
    SetError("Failed to encode public key.");
    return nullptr;
  }

  Ukey2ServerInit server_init;
  server_init.set_version(kVersion);
  server_init.set_random(nonce->String());
  server_init.set_handshake_cipher(
      static_cast<Ukey2HandshakeCipher>(handshake_cipher_));
  server_init.set_public_key(public_key->SerializeAsString());

  return MakeUkey2Message(Ukey2Message::SERVER_INIT,
                          server_init.SerializeAsString());
}

// Generates the serialized representation of a Ukey2Message based on the
// provided |type| and |data|. On error, returns nullptr and writes error
// message to |out_error|.
std::unique_ptr<string> UKey2Handshake::MakeUkey2Message(
    Ukey2Message::Type type, const string& data) {
  Ukey2Message message;
  if (!Ukey2Message::Type_IsValid(type)) {
    std::ostringstream stream;
    stream << "Invalid message type: " << type;
    SetError(stream.str());
    return nullptr;
  }
  message.set_message_type(type);

  // Only ALERT messages can have a blank data field.
  if (type != Ukey2Message::ALERT) {
    if (data.length() == 0) {
      SetError("Cannot send empty message data for non-alert messages");
      return nullptr;
    }
  }
  message.set_message_data(data);

  std::unique_ptr<string> serialized(new string());
  message.SerializeToString(serialized.get());
  return serialized;
}

void UKey2Handshake::SetError(const string& error_message) {
  handshake_state_ = InternalState::HANDSHAKE_ERROR;
  last_error_ = error_message;
}

}  // namespace securegcm
