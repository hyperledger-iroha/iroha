import Foundation

// C ABI imports (ensure NoritoBridge.xcframework is linked and module.modulemap is present)
@_silgen_name("connect_norito_encode_ciphertext_frame")
func ffi_encode_frame(_ sid: UnsafePointer<UInt8>, _ dir: UInt8, _ seq: UInt64, _ aead: UnsafePointer<UInt8>, _ aeadLen: CUnsignedLong, _ outPtr: UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, _ outLen: UnsafeMutablePointer<CUnsignedLong>) -> Int32
@_silgen_name("connect_norito_decode_ciphertext_frame")
func ffi_decode_frame(_ inp: UnsafePointer<UInt8>, _ inpLen: CUnsignedLong, _ outSid: UnsafeMutablePointer<UInt8>, _ outDir: UnsafeMutablePointer<UInt8>, _ outSeq: UnsafeMutablePointer<UInt64>, _ outAeadPtr: UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, _ outAeadLen: UnsafeMutablePointer<CUnsignedLong>) -> Int32
@_silgen_name("connect_norito_encode_envelope_sign_request_tx")
func ffi_env_sreq_tx(_ seq: UInt64, _ tx: UnsafePointer<UInt8>, _ txLen: CUnsignedLong, _ outPtr: UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, _ outLen: UnsafeMutablePointer<CUnsignedLong>) -> Int32
@_silgen_name("connect_norito_encode_envelope_sign_request_raw")
func ffi_env_sreq_raw(_ seq: UInt64, _ tag: UnsafePointer<UInt8>, _ tagLen: CUnsignedLong, _ bytes: UnsafePointer<UInt8>, _ bytesLen: CUnsignedLong, _ outPtr: UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, _ outLen: UnsafeMutablePointer<CUnsignedLong>) -> Int32
@_silgen_name("connect_norito_encode_envelope_sign_result_ok")
func ffi_env_sres_ok(_ seq: UInt64, _ sig: UnsafePointer<UInt8>, _ sigLen: CUnsignedLong, _ outPtr: UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, _ outLen: UnsafeMutablePointer<CUnsignedLong>) -> Int32
@_silgen_name("connect_norito_encode_envelope_sign_result_ok_with_alg")
func ffi_env_sres_ok_alg(_ seq: UInt64, _ algPtr: UnsafePointer<CChar>, _ algLen: CUnsignedLong, _ sigPtr: UnsafePointer<UInt8>, _ sigLen: CUnsignedLong, _ outPtr: UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, _ outLen: UnsafeMutablePointer<CUnsignedLong>) -> Int32
@_silgen_name("connect_norito_encode_envelope_sign_result_err")
func ffi_env_sres_err(_ seq: UInt64, _ code: UnsafePointer<UInt8>, _ codeLen: CUnsignedLong, _ message: UnsafePointer<UInt8>, _ messageLen: CUnsignedLong, _ outPtr: UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, _ outLen: UnsafeMutablePointer<CUnsignedLong>) -> Int32
@_silgen_name("connect_norito_encode_envelope_control_close")
func ffi_env_close(_ seq: UInt64, _ who: UInt8, _ code: UInt16, _ reason: UnsafePointer<UInt8>, _ reasonLen: CUnsignedLong, _ retryable: UInt8, _ outPtr: UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, _ outLen: UnsafeMutablePointer<CUnsignedLong>) -> Int32
@_silgen_name("connect_norito_encode_envelope_control_reject")
func ffi_env_reject(_ seq: UInt64, _ code: UInt16, _ codeId: UnsafePointer<UInt8>, _ codeIdLen: CUnsignedLong, _ reason: UnsafePointer<UInt8>, _ reasonLen: CUnsignedLong, _ outPtr: UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, _ outLen: UnsafeMutablePointer<CUnsignedLong>) -> Int32
@_silgen_name("connect_norito_decode_envelope_kind")
func ffi_env_kind(_ inp: UnsafePointer<UInt8>, _ len: CUnsignedLong, _ outSeq: UnsafeMutablePointer<UInt64>, _ outKind: UnsafeMutablePointer<UInt16>) -> Int32
@_silgen_name("connect_norito_decode_envelope_json")
func ffi_env_json(_ inp: UnsafePointer<UInt8>, _ len: CUnsignedLong, _ outPtr: UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, _ outLen: UnsafeMutablePointer<CUnsignedLong>) -> Int32
@_silgen_name("connect_norito_decode_envelope_sign_result_alg")
func ffi_env_sres_alg(_ inp: UnsafePointer<UInt8>, _ len: CUnsignedLong, _ outPtr: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>, _ outLen: UnsafeMutablePointer<CUnsignedLong>) -> Int32
@_silgen_name("connect_norito_free")
func ffi_free(_ ptr: UnsafeMutablePointer<UInt8>?)
@_silgen_name("connect_norito_sorafs_local_fetch")
func ffi_sorafs_local_fetch(
  _ planPtr: UnsafePointer<CChar>?, _ planLen: CUnsignedLong,
  _ providersPtr: UnsafePointer<CChar>?, _ providersLen: CUnsignedLong,
  _ optionsPtr: UnsafePointer<CChar>?, _ optionsLen: CUnsignedLong,
  _ outPayloadPtr: UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, _ outPayloadLen: UnsafeMutablePointer<CUnsignedLong>?,
  _ outReportPtr: UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, _ outReportLen: UnsafeMutablePointer<CUnsignedLong>?
) -> Int32

public enum NoritoError: Error { case ffi(Int32) }

public final class NoritoBridgeKit {
  public init() {}

  // This helper focuses on the FFI surface only. Higher-level WebSocket flows
  // (ConnectClient/ConnectSession, ChaChaPoly seal/open, token handling) live in
  // the main `IrohaSwift` package so apps can work with typed envelopes instead
  // of juggling raw frames.

  private func wrapBytes(_ data: Data, _ body: (UnsafePointer<UInt8>, CUnsignedLong) throws -> Data) rethrows -> Data {
    return try data.withUnsafeBytes { (bp: UnsafeRawBufferPointer) -> Data in
      let b = bp.bindMemory(to: UInt8.self)
      return try body(b.baseAddress!, CUnsignedLong(data.count))
    }
  }

  public func encodeCiphertextFrame(sid: Data, dir: UInt8, seq: UInt64, aead: Data) throws -> Data {
    precondition(sid.count == 32)
    var outPtr: UnsafeMutablePointer<UInt8>? = nil
    var outLen: CUnsignedLong = 0
    let rc = sid.withUnsafeBytes { sp in
      aead.withUnsafeBytes { ap in
        ffi_encode_frame(sp.bindMemory(to: UInt8.self).baseAddress!, dir, seq, ap.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(aead.count), &outPtr, &outLen)
      }
    }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let d = Data(bytes: p, count: Int(outLen)); ffi_free(p); return d
  }

  public func decodeCiphertextFrame(_ frame: Data) throws -> (sid: Data, dir: UInt8, seq: UInt64, aead: Data) {
    var sidOut = Data(count: 32); var dirOut: UInt8 = 0; var seqOut: UInt64 = 0
    var aeadPtr: UnsafeMutablePointer<UInt8>? = nil; var aeadLen: CUnsignedLong = 0
    let rc = frame.withUnsafeBytes { fp in
      sidOut.withUnsafeMutableBytes { sp in
        ffi_decode_frame(fp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(frame.count), sp.bindMemory(to: UInt8.self).baseAddress!, &dirOut, &seqOut, &aeadPtr, &aeadLen)
      }
    }
    guard rc == 0, let ap = aeadPtr else { throw NoritoError.ffi(rc) }
    let aead = Data(bytes: ap, count: Int(aeadLen)); ffi_free(ap)
    return (sid: sidOut, dir: dirOut, seq: seqOut, aead: aead)
  }

  // Envelope encoders
  public func encodeEnvelopeSignRequestTx(seq: UInt64, tx: Data) throws -> Data {
    var outPtr: UnsafeMutablePointer<UInt8>? = nil; var outLen: CUnsignedLong = 0
    let rc = tx.withUnsafeBytes { tp in ffi_env_sreq_tx(seq, tp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(tx.count), &outPtr, &outLen) }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let d = Data(bytes: p, count: Int(outLen)); ffi_free(p); return d
  }
  public func encodeEnvelopeSignRequestRaw(seq: UInt64, domainTag: String, bytes: Data) throws -> Data {
    var outPtr: UnsafeMutablePointer<UInt8>? = nil; var outLen: CUnsignedLong = 0
    let rc = domainTag.utf8CString.withUnsafeBufferPointer { tb in
      let td = Data(tb.dropLast())
      return td.withUnsafeBytes { tpb in
        bytes.withUnsafeBytes { bp in
          ffi_env_sreq_raw(seq, tpb.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(td.count), bp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(bytes.count), &outPtr, &outLen)
        }
      }
    }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let d = Data(bytes: p, count: Int(outLen)); ffi_free(p); return d
  }
  public func encodeEnvelopeSignResultOk(seq: UInt64, signature: Data) throws -> Data {
    var outPtr: UnsafeMutablePointer<UInt8>? = nil; var outLen: CUnsignedLong = 0
    let rc = signature.withUnsafeBytes { sp in ffi_env_sres_ok(seq, sp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(signature.count), &outPtr, &outLen) }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let d = Data(bytes: p, count: Int(outLen)); ffi_free(p); return d
  }
  public func encodeEnvelopeSignResultOk(seq: UInt64, algorithm: String, signature: Data) throws -> Data {
    var outPtr: UnsafeMutablePointer<UInt8>? = nil; var outLen: CUnsignedLong = 0
    let algLen = CUnsignedLong(algorithm.lengthOfBytes(using: .utf8))
    let rc = signature.withUnsafeBytes { sp in
      algorithm.withCString { cAlg in
        ffi_env_sres_ok_alg(seq, cAlg, algLen, sp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(signature.count), &outPtr, &outLen)
      }
    }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let d = Data(bytes: p, count: Int(outLen)); ffi_free(p); return d
  }
  public func encodeEnvelopeSignResultErr(seq: UInt64, code: String, message: String) throws -> Data {
    var outPtr: UnsafeMutablePointer<UInt8>? = nil; var outLen: CUnsignedLong = 0
    let rc = code.utf8CString.withUnsafeBufferPointer { cb in
      let cd = Data(cb.dropLast())
      return cd.withUnsafeBytes { c in
        message.utf8CString.withUnsafeBufferPointer { mb in
          let md = Data(mb.dropLast())
          return md.withUnsafeBytes { m in ffi_env_sres_err(seq, c.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(cd.count), m.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(md.count), &outPtr, &outLen) }
        }
      }
    }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let d = Data(bytes: p, count: Int(outLen)); ffi_free(p); return d
  }
  public func encodeEnvelopeClose(seq: UInt64, who: UInt8, code: UInt16, reason: String, retryable: Bool) throws -> Data {
    var outPtr: UnsafeMutablePointer<UInt8>? = nil; var outLen: CUnsignedLong = 0
    let rc = reason.utf8CString.withUnsafeBufferPointer { rb in
      let rd = Data(rb.dropLast())
      return rd.withUnsafeBytes { r in ffi_env_close(seq, who, code, r.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(rd.count), retryable ? 1 : 0, &outPtr, &outLen) }
    }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let d = Data(bytes: p, count: Int(outLen)); ffi_free(p); return d
  }
  public func encodeEnvelopeReject(seq: UInt64, code: UInt16, codeId: String, reason: String) throws -> Data {
    var outPtr: UnsafeMutablePointer<UInt8>? = nil; var outLen: CUnsignedLong = 0
    let rc = codeId.utf8CString.withUnsafeBufferPointer { ib in
      let id = Data(ib.dropLast())
      return id.withUnsafeBytes { i in
        reason.utf8CString.withUnsafeBufferPointer { rb in
          let rd = Data(rb.dropLast())
          return rd.withUnsafeBytes { r in ffi_env_reject(seq, code, i.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(id.count), r.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(rd.count), &outPtr, &outLen) }
        }
      }
    }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let d = Data(bytes: p, count: Int(outLen)); ffi_free(p); return d
  }

  // Envelope decoders
  public func decodeEnvelopeKind(_ env: Data) throws -> (seq: UInt64, kind: UInt16) {
    var seq: UInt64 = 0; var kind: UInt16 = 0
    let rc = env.withUnsafeBytes { ep in ffi_env_kind(ep.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(env.count), &seq, &kind) }
    guard rc == 0 else { throw NoritoError.ffi(rc) }
    return (seq, kind)
  }
  public func decodeEnvelopeJson(_ env: Data) throws -> String {
    var outPtr: UnsafeMutablePointer<UInt8>? = nil; var outLen: CUnsignedLong = 0
    let rc = env.withUnsafeBytes { ep in ffi_env_json(ep.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(env.count), &outPtr, &outLen) }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let data = Data(bytes: p, count: Int(outLen)); ffi_free(p)
    return String(data: data, encoding: .utf8) ?? "{}"
  }
  public func decodeSignResultAlgorithm(_ env: Data) throws -> String {
    var outPtr: UnsafeMutablePointer<CChar>? = nil; var outLen: CUnsignedLong = 0
    let rc = env.withUnsafeBytes { ep in ffi_env_sres_alg(ep.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(env.count), &outPtr, &outLen) }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let data = Data(bytes: UnsafeRawPointer(p), count: Int(outLen))
    ffi_free(UnsafeMutableRawPointer(p).assumingMemoryBound(to: UInt8.self))
    return String(data: data, encoding: .utf8) ?? ""
  }

  public func sorafsLocalFetch(planJSON: String,
                               providersJSON: String,
                               optionsJSON: String? = nil) throws -> (payload: Data, reportJSON: String) {
    let planData = planJSON.data(using: .utf8) ?? Data()
    let providersData = providersJSON.data(using: .utf8) ?? Data()
    let optionsData = optionsJSON?.data(using: .utf8) ?? Data()

    var payloadPtr: UnsafeMutablePointer<UInt8>? = nil
    var payloadLen: CUnsignedLong = 0
    var reportPtr: UnsafeMutablePointer<UInt8>? = nil
    var reportLen: CUnsignedLong = 0

    let rc = planData.withUnsafeBytes { planBuf in
      providersData.withUnsafeBytes { providersBuf in
        optionsData.withUnsafeBytes { optionsBuf in
          ffi_sorafs_local_fetch(
            planBuf.bindMemory(to: CChar.self).baseAddress, CUnsignedLong(planBuf.count),
            providersBuf.bindMemory(to: CChar.self).baseAddress, CUnsignedLong(providersBuf.count),
            optionsBuf.bindMemory(to: CChar.self).baseAddress, CUnsignedLong(optionsBuf.count),
            &payloadPtr, &payloadLen,
            &reportPtr, &reportLen
          )
        }
      }
    }

    if rc != 0 {
      if let payloadPtr { ffi_free(payloadPtr) }
      if let reportPtr { ffi_free(reportPtr) }
      throw NoritoError.ffi(rc)
    }

    guard let payloadPtr else { throw NoritoError.ffi(rc) }
    let payload = Data(bytes: payloadPtr, count: Int(payloadLen))
    ffi_free(payloadPtr)

    guard let reportPtr else { throw NoritoError.ffi(rc) }
    let reportData = Data(bytes: reportPtr, count: Int(reportLen))
    ffi_free(reportPtr)
    guard let reportString = String(data: reportData, encoding: .utf8) else {
      throw NoritoError.ffi(-100)
    }
    return (payload, reportString)
  }
}
