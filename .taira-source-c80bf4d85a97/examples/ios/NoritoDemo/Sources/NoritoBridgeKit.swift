// This file is conditionally compiled when NoritoBridge.xcframework is linked.
#if canImport(NoritoBridge)
import Foundation
import NoritoBridge

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
@_silgen_name("connect_norito_free")
func ffi_free(_ ptr: UnsafeMutablePointer<UInt8>?)

public enum NoritoError: Error { case ffi(Int32) }

public final class NoritoBridgeKit {
  public init() {}
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

  // Optional Control frame helpers (via dlsym; throw if unavailable)
  public func encodeControlOpen(sid: Data, dir: UInt8, seq: UInt64, appPub: Data) throws -> Data {
    try controlEncode4(name: "connect_norito_encode_control_open", sid: sid, dir: dir, seq: seq, p1: appPub)
  }
  public func encodeControlApprove(sid: Data, dir: UInt8, seq: UInt64, walletPub: Data, account: Data? = nil, sig: Data? = nil) throws -> Data {
    let sym = dlsym(RTLD_DEFAULT, "connect_norito_encode_control_approve")
    guard let s = sym else { throw NoritoError.ffi(-1) }
    typealias Fn = @convention(c) (UnsafePointer<UInt8>, UInt8, UInt64, UnsafePointer<UInt8>, CUnsignedLong, UnsafePointer<UInt8>?, CUnsignedLong, UnsafePointer<UInt8>?, CUnsignedLong, UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, UnsafeMutablePointer<CUnsignedLong>) -> Int32
    let fn = unsafeBitCast(s, to: Fn.self)
    var outPtr: UnsafeMutablePointer<UInt8>? = nil; var outLen: CUnsignedLong = 0
    let rc = sid.withUnsafeBytes { sp in
      walletPub.withUnsafeBytes { wp in
        (account ?? Data()).withUnsafeBytes { ap in
          (sig ?? Data()).withUnsafeBytes { sg in
            fn(sp.bindMemory(to: UInt8.self).baseAddress!, dir, seq,
               wp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(walletPub.count),
               account == nil ? nil : ap.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(account?.count ?? 0),
               sig == nil ? nil : sg.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(sig?.count ?? 0),
               &outPtr, &outLen)
          }
        }
      }
    }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let d = Data(bytes: p, count: Int(outLen)); ffi_free(p); return d
  }
  public func decodeControlKind(_ frame: Data) throws -> (sid: Data, dir: UInt8, seq: UInt64, kind: UInt16) {
    guard let sym = dlsym(RTLD_DEFAULT, "connect_norito_decode_control_kind") else { throw NoritoError.ffi(-1) }
    typealias Fn = @convention(c) (UnsafePointer<UInt8>, CUnsignedLong, UnsafeMutablePointer<UInt8>, UnsafeMutablePointer<UInt8>, UnsafeMutablePointer<UInt64>, UnsafeMutablePointer<UInt16>) -> Int32
    let fn = unsafeBitCast(sym, to: Fn.self)
    var sidOut = Data(count: 32); var dir: UInt8 = 0; var seq: UInt64 = 0; var kind: UInt16 = 0
    let rc = frame.withUnsafeBytes { fp in
      sidOut.withUnsafeMutableBytes { sp in
        fn(fp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(frame.count), sp.bindMemory(to: UInt8.self).baseAddress!, &dir, &seq, &kind)
      }
    }
    guard rc == 0 else { throw NoritoError.ffi(rc) }
    return (sid: sidOut, dir: dir, seq: seq, kind: kind)
  }
  public func decodeControlOpenPub(_ frame: Data) throws -> Data {
    guard let sym = dlsym(RTLD_DEFAULT, "connect_norito_decode_control_open_pub") else { throw NoritoError.ffi(-1) }
    typealias Fn = @convention(c) (UnsafePointer<UInt8>, CUnsignedLong, UnsafeMutablePointer<UInt8>) -> Int32
    let fn = unsafeBitCast(sym, to: Fn.self)
    var out = Data(count: 32)
    let rc = frame.withUnsafeBytes { fp in
      out.withUnsafeMutableBytes { op in fn(fp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(frame.count), op.bindMemory(to: UInt8.self).baseAddress!) }
    }
    guard rc == 0 else { throw NoritoError.ffi(rc) }
    return out
  }
  public func decodeControlApprovePub(_ frame: Data) throws -> Data {
    guard let sym = dlsym(RTLD_DEFAULT, "connect_norito_decode_control_approve_pub") else { throw NoritoError.ffi(-1) }
    typealias Fn = @convention(c) (UnsafePointer<UInt8>, CUnsignedLong, UnsafeMutablePointer<UInt8>) -> Int32
    let fn = unsafeBitCast(sym, to: Fn.self)
    var out = Data(count: 32)
    let rc = frame.withUnsafeBytes { fp in
      out.withUnsafeMutableBytes { op in fn(fp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(frame.count), op.bindMemory(to: UInt8.self).baseAddress!) }
    }
    guard rc == 0 else { throw NoritoError.ffi(rc) }
    return out
  }
  public func decodeControlApproveAccount(_ frame: Data) throws -> Data {
    guard let sym = dlsym(RTLD_DEFAULT, "connect_norito_decode_control_approve_account") else { throw NoritoError.ffi(-1) }
    typealias Fn = @convention(c) (UnsafePointer<UInt8>, CUnsignedLong, UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, UnsafeMutablePointer<CUnsignedLong>) -> Int32
    let fn = unsafeBitCast(sym, to: Fn.self)
    var ptr: UnsafeMutablePointer<UInt8>? = nil; var len: CUnsignedLong = 0
    let rc = frame.withUnsafeBytes { fp in
      fn(fp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(frame.count), &ptr, &len)
    }
    guard rc == 0, let p = ptr else { throw NoritoError.ffi(rc) }
    let d = Data(bytes: p, count: Int(len)); ffi_free(p); return d
  }
  public func decodeControlApproveSig(_ frame: Data) throws -> Data {
    guard let sym = dlsym(RTLD_DEFAULT, "connect_norito_decode_control_approve_sig") else { throw NoritoError.ffi(-1) }
    typealias Fn = @convention(c) (UnsafePointer<UInt8>, CUnsignedLong, UnsafeMutablePointer<UInt8>) -> Int32
    let fn = unsafeBitCast(sym, to: Fn.self)
    var out = Data(count: 64)
    let rc = frame.withUnsafeBytes { fp in
      out.withUnsafeMutableBytes { op in fn(fp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(frame.count), op.bindMemory(to: UInt8.self).baseAddress!) }
    }
    guard rc == 0 else { throw NoritoError.ffi(rc) }
    return out
  }

  public func decodeControlApproveAccountJson(_ frame: Data) throws -> String {
    guard let sym = dlsym(RTLD_DEFAULT, "connect_norito_decode_control_approve_account_json") else { throw NoritoError.ffi(-1) }
    typealias Fn = @convention(c) (UnsafePointer<UInt8>, CUnsignedLong, UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, UnsafeMutablePointer<CUnsignedLong>) -> Int32
    let fn = unsafeBitCast(sym, to: Fn.self)
    var outPtr: UnsafeMutablePointer<UInt8>? = nil; var outLen: CUnsignedLong = 0
    let rc = frame.withUnsafeBytes { fp in
      fn(fp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(frame.count), &outPtr, &outLen)
    }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let data = Data(bytes: p, count: Int(outLen)); ffi_free(p)
    return String(data: data, encoding: .utf8) ?? "{}"
  }

  public func decodeControlOpenPermissionsJson(_ frame: Data) throws -> String {
    guard let sym = dlsym(RTLD_DEFAULT, "connect_norito_decode_control_open_permissions_json") else { throw NoritoError.ffi(-1) }
    typealias Fn = @convention(c) (UnsafePointer<UInt8>, CUnsignedLong, UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, UnsafeMutablePointer<CUnsignedLong>) -> Int32
    let fn = unsafeBitCast(sym, to: Fn.self)
    var outPtr: UnsafeMutablePointer<UInt8>? = nil; var outLen: CUnsignedLong = 0
    let rc = frame.withUnsafeBytes { fp in fn(fp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(frame.count), &outPtr, &outLen) }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let data = Data(bytes: p, count: Int(outLen)); ffi_free(p)
    return String(data: data, encoding: .utf8) ?? "{}"
  }
  public func decodeControlApprovePermissionsJson(_ frame: Data) throws -> String {
    guard let sym = dlsym(RTLD_DEFAULT, "connect_norito_decode_control_approve_permissions_json") else { throw NoritoError.ffi(-1) }
    typealias Fn = @convention(c) (UnsafePointer<UInt8>, CUnsignedLong, UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, UnsafeMutablePointer<CUnsignedLong>) -> Int32
    let fn = unsafeBitCast(sym, to: Fn.self)
    var outPtr: UnsafeMutablePointer<UInt8>? = nil; var outLen: CUnsignedLong = 0
    let rc = frame.withUnsafeBytes { fp in fn(fp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(frame.count), &outPtr, &outLen) }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let data = Data(bytes: p, count: Int(outLen)); ffi_free(p)
    return String(data: data, encoding: .utf8) ?? "{}"
  }

  public func encodeControlOpenExt(sid: Data, dir: UInt8, seq: UInt64, appPub: Data, appMetaJson: Data?, chainId: String, permissionsJson: Data?) throws -> Data {
    guard let sym = dlsym(RTLD_DEFAULT, "connect_norito_encode_control_open_ext") else { throw NoritoError.ffi(-1) }
    typealias Fn = @convention(c) (UnsafePointer<UInt8>, UInt8, UInt64, UnsafePointer<UInt8>, CUnsignedLong, UnsafePointer<UInt8>?, CUnsignedLong, UnsafePointer<CChar>, UnsafePointer<UInt8>?, CUnsignedLong, UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, UnsafeMutablePointer<CUnsignedLong>) -> Int32
    let fn = unsafeBitCast(sym, to: Fn.self)
    var outPtr: UnsafeMutablePointer<UInt8>? = nil; var outLen: CUnsignedLong = 0
    let rc = sid.withUnsafeBytes { sp in
      appPub.withUnsafeBytes { ap in
        (appMetaJson ?? Data()).withUnsafeBytes { mj in
          (permissionsJson ?? Data()).withUnsafeBytes { pj in
            chainId.utf8CString.withUnsafeBufferPointer { cb in
              fn(sp.bindMemory(to: UInt8.self).baseAddress!, dir, seq,
                 ap.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(appPub.count),
                 appMetaJson == nil ? nil : mj.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(appMetaJson?.count ?? 0),
                 cb.baseAddress!,
                 permissionsJson == nil ? nil : pj.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(permissionsJson?.count ?? 0),
                 &outPtr, &outLen)
            }
          }
        }
      }
    }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let d = Data(bytes: p, count: Int(outLen)); ffi_free(p); return d
  }

  public func encodeControlApproveExt(sid: Data, dir: UInt8, seq: UInt64, walletPub: Data, accountId: String, permissionsJson: Data?, proofJson: Data?, sig: Data) throws -> Data {
    guard let sym = dlsym(RTLD_DEFAULT, "connect_norito_encode_control_approve_ext") else { throw NoritoError.ffi(-1) }
    typealias Fn = @convention(c) (UnsafePointer<UInt8>, UInt8, UInt64, UnsafePointer<UInt8>, CUnsignedLong, UnsafePointer<CChar>, UnsafePointer<UInt8>?, CUnsignedLong, UnsafePointer<UInt8>?, CUnsignedLong, UnsafePointer<UInt8>, CUnsignedLong, UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, UnsafeMutablePointer<CUnsignedLong>) -> Int32
    let fn = unsafeBitCast(sym, to: Fn.self)
    var outPtr: UnsafeMutablePointer<UInt8>? = nil; var outLen: CUnsignedLong = 0
    let rc = sid.withUnsafeBytes { sp in
      walletPub.withUnsafeBytes { wp in
        (permissionsJson ?? Data()).withUnsafeBytes { pj in
          (proofJson ?? Data()).withUnsafeBytes { rj in
            sig.withUnsafeBytes { sg in
              accountId.utf8CString.withUnsafeBufferPointer { ab in
                fn(sp.bindMemory(to: UInt8.self).baseAddress!, dir, seq,
                   wp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(walletPub.count),
                   ab.baseAddress!,
                   permissionsJson == nil ? nil : pj.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(permissionsJson?.count ?? 0),
                   proofJson == nil ? nil : rj.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(proofJson?.count ?? 0),
                   sg.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(sig.count),
                   &outPtr, &outLen)
              }
            }
          }
        }
      }
    }
    guard rc == 0, let p = outPtr else { throw NoritoError.ffi(rc) }
    let d = Data(bytes: p, count: Int(outLen)); ffi_free(p); return d
  }
  private func controlEncode4(name: String, sid: Data, dir: UInt8, seq: UInt64, p1: Data) throws -> Data {
    guard let sym = dlsym(RTLD_DEFAULT, name) else { throw NoritoError.ffi(-1) }
    typealias Fn = @convention(c) (UnsafePointer<UInt8>, UInt8, UInt64, UnsafePointer<UInt8>, CUnsignedLong, UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>, UnsafeMutablePointer<CUnsignedLong>) -> Int32
    let fn = unsafeBitCast(sym, to: Fn.self)
    var outPtr: UnsafeMutablePointer<UInt8>? = nil; var outLen: CUnsignedLong = 0
    let rc = sid.withUnsafeBytes { sp in
      p1.withUnsafeBytes { p in fn(sp.bindMemory(to: UInt8.self).baseAddress!, dir, seq, p.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(p1.count), &outPtr, &outLen) }
    }
    guard rc == 0, let ptr = outPtr else { throw NoritoError.ffi(rc) }
    let d = Data(bytes: ptr, count: Int(outLen)); ffi_free(ptr); return d
  }
}
#else
import Foundation
public final class NoritoBridgeKit { public init() {} }
#endif
