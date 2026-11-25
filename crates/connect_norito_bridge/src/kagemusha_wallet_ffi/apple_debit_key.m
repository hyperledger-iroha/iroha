// Source-owned Secure Enclave key creation, lookup and per-use signing for one admitted wallet.
#import <Foundation/Foundation.h>
#import <LocalAuthentication/LocalAuthentication.h>
#import <Security/Security.h>
#import <TargetConditionals.h>

#if !TARGET_OS_SIMULATOR && !TARGET_OS_MACCATALYST
#ifdef IROHA_SPARTAN_DEVICE_FIXTURE
#import <CommonCrypto/CommonDigest.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

// Development-only diagnostic namespace: the single implementation shared with FixtureCustody.swift
// (custody, payment-key prefix, rollback anchor and report) through spartan_device_fixture.h. The
// namespace is SHA-256 of this preimage; the array's terminating NUL is the 0x00 separator:
//   "kagemusha-spartan-fixture-build-namespace-v1" || 0x00
//   || SHA-256(signed main executable file) || SHA-256(native-device/parameter-manifest.norito)
// The signed executable is each build's immutable identity, so builds embedding the same signed
// manifest never share keys, and relaunching or reinstalling one build restores its own.
static const char fixtureNamespaceDomain[] = "kagemusha-spartan-fixture-build-namespace-v1";
#define FIXTURE_PREIMAGE_BYTES (sizeof(fixtureNamespaceDomain) + 2 * CC_SHA256_DIGEST_LENGTH)

// Bounded streaming SHA-256 of a non-empty regular file of at most `limit` bytes.
static BOOL fixtureFileDigest(NSString *path, off_t limit, uint8_t digest[CC_SHA256_DIGEST_LENGTH]) {
    int file = path ? open(path.fileSystemRepresentation, O_RDONLY | O_CLOEXEC) : -1;
    if (file < 0) return NO;
    struct stat status;
    BOOL valid = !fstat(file, &status) && S_ISREG(status.st_mode) && status.st_size > 0 && status.st_size <= limit;
    if (valid) {
        NSMutableData *buffer = [NSMutableData dataWithLength:1 << 20];
        CC_SHA256_CTX context;
        CC_SHA256_Init(&context);
        off_t total = 0;
        ssize_t count;
        while ((count = read(file, buffer.mutableBytes, buffer.length)) > 0 && count <= status.st_size - total) {
            CC_SHA256_Update(&context, buffer.bytes, (CC_LONG)count);
            total += count;
        }
        CC_SHA256_Final(digest, &context);
        valid = !count && total == status.st_size;
    }
    close(file);
    return valid;
}

static BOOL fixturePreimageFor(NSString *executable, NSString *manifest, uint8_t preimage[FIXTURE_PREIMAGE_BYTES]) {
    memcpy(preimage, fixtureNamespaceDomain, sizeof(fixtureNamespaceDomain));
    uint8_t *digests = preimage + sizeof(fixtureNamespaceDomain);
    // InstallKagemushaSpartanProgramsV1::MAX_MANIFEST_BYTES bounds the manifest.
    return fixtureFileDigest(executable, (off_t)1 << 30, digests) &&
           fixtureFileDigest(manifest, 16 * 1024, digests + CC_SHA256_DIGEST_LENGTH);
}

static const uint8_t *fixturePreimage(void) {
    static uint8_t preimage[FIXTURE_PREIMAGE_BYTES];
    static BOOL ready;
    static dispatch_once_t load;
    dispatch_once(&load, ^{
        NSBundle *bundle = NSBundle.mainBundle;
        ready = fixturePreimageFor(bundle.executablePath,
            [bundle pathForResource:@"parameter-manifest" ofType:@"norito" inDirectory:@"native-device"], preimage);
    });
    return ready ? preimage : NULL;
}

static NSString *fixtureNamespace(void) {
    static NSString *scope;
    static dispatch_once_t load;
    dispatch_once(&load, ^{
        const uint8_t *preimage = fixturePreimage();
        if (!preimage) return;
        uint8_t hash[CC_SHA256_DIGEST_LENGTH];
        CC_SHA256(preimage, (CC_LONG)FIXTURE_PREIMAGE_BYTES, hash);
        NSMutableString *hex = [NSMutableString stringWithCapacity:2 * sizeof(hash)];
        for (size_t i = 0; i < sizeof(hash); i++) [hex appendFormat:@"%02x", hash[i]];
        scope = hex;
    });
    return scope;
}
#endif

static NSData *keyTag(const uint8_t slot[32], const uint8_t *nonce, NSString *scope) {
#ifdef IROHA_SPARTAN_DEVICE_FIXTURE
    if (!scope) return nil;
    NSMutableString *name = [NSMutableString stringWithFormat:
        (nonce ? @"kgm-ns-debit1-%@-" : @"kgm-ns-w1-%@-"), scope];
#else
    NSMutableString *name = [(nonce ? @"kgm-debit1-" : @"kgm-w1-") mutableCopy];
#endif
    for (size_t i = 0; i < 32; i++) [name appendFormat:@"%02x", slot[i]];
    if (nonce) {
        [name appendString:@"-"];
        for (size_t i = 0; i < 32; i++) [name appendFormat:@"%02x", nonce[i]];
    }
    return [name dataUsingEncoding:NSUTF8StringEncoding];
}

static OSStatus osError(CFErrorRef error) {
    NSError *e = CFBridgingRelease(error);
    if ([e.domain isEqualToString:NSOSStatusErrorDomain] && e.code != errSecSuccess)
        return (OSStatus)e.code;
    if ([e.domain isEqualToString:LAErrorDomain]) {
        switch (e.code) {
            case LAErrorUserCancel: case LAErrorAppCancel: case LAErrorSystemCancel:
                return errSecUserCanceled;
            case LAErrorNotInteractive: return errSecInteractionNotAllowed;
        }
    }
    return errSecAuthFailed;
}

static OSStatus uniqueEnclaveItem(NSData *tag, NSString *group, LAContext *context,
                                  BOOL returnKey, NSDictionary **item) {
    NSMutableDictionary *query = [@{
        (__bridge id)kSecClass: (__bridge id)kSecClassKey,
        (__bridge id)kSecAttrKeyClass: (__bridge id)kSecAttrKeyClassPrivate,
        (__bridge id)kSecAttrKeyType: (__bridge id)kSecAttrKeyTypeECSECPrimeRandom,
        (__bridge id)kSecAttrTokenID: (__bridge id)kSecAttrTokenIDSecureEnclave,
        (__bridge id)kSecAttrApplicationTag: tag,
        (__bridge id)kSecUseDataProtectionKeychain: @YES,
        (__bridge id)kSecReturnAttributes: @YES,
        (__bridge id)kSecMatchLimit: (__bridge id)kSecMatchLimitAll,
        (__bridge id)kSecUseAuthenticationContext: context,
    } mutableCopy];
    if (returnKey) query[(__bridge id)kSecReturnRef] = @YES;
    if (group) query[(__bridge id)kSecAttrAccessGroup] = group;
    CFTypeRef result = NULL;
    OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)query, &result);
    id found = CFBridgingRelease(result);
    if (status != errSecSuccess) return status;
    if (![found isKindOfClass:NSArray.class] || [found count] != 1) return errSecInvalidKeyRef;
    id attributes = [found firstObject];
    if (![attributes isKindOfClass:NSDictionary.class]) return errSecDecode;
    *item = attributes;
    return errSecSuccess;
}

static OSStatus approvalKey(const uint8_t slot[32], const uint8_t nonce[32],
                            BOOL create, LAContext *context, SecKeyRef *result) {
    if (NSProcessInfo.processInfo.isiOSAppOnMac) return errSecNotAvailable;
    NSString *scope = nil;
#ifdef IROHA_SPARTAN_DEVICE_FIXTURE
    scope = fixtureNamespace();
    if (!scope) return errSecDecode;
#endif
    LAContext *probe = [[LAContext alloc] init];
    probe.interactionNotAllowed = YES;
    NSDictionary *payment = nil;
    OSStatus status = uniqueEnclaveItem(keyTag(slot, NULL, scope), nil, probe, NO, &payment);
    [probe invalidate];
    if (status != errSecSuccess) return status;
    NSString *group = payment[(__bridge id)kSecAttrAccessGroup];
    NSString *bundle = NSBundle.mainBundle.bundleIdentifier;
    if (![group isKindOfClass:NSString.class] || !bundle.length) return errSecMissingEntitlement;
    NSRange split = [group rangeOfString:@"."];
    if (split.location != 10 || ![[group substringFromIndex:NSMaxRange(split)] isEqualToString:bundle])
        return errSecMissingEntitlement;
    for (NSUInteger i = 0; i < split.location; i++) {
        unichar c = [group characterAtIndex:i];
        if (!((c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9'))) return errSecMissingEntitlement;
    }
    NSData *tag = keyTag(slot, nonce, scope);
    NSDictionary *approval = nil;
    status = uniqueEnclaveItem(tag, group, context, !create, &approval);
    if (!create) {
        if (status != errSecSuccess) return status;
        id object = approval[(__bridge id)kSecValueRef];
        if (!object || CFGetTypeID((__bridge CFTypeRef)object) != SecKeyGetTypeID())
            return errSecInvalidKeyRef;
        *result = (SecKeyRef)CFRetain((__bridge CFTypeRef)object);
        return errSecSuccess;
    }
    if (status != errSecItemNotFound) return status == errSecSuccess ? errSecDuplicateItem : status;
    CFErrorRef error = NULL;
    SecAccessControlRef acl = SecAccessControlCreateWithFlags(NULL, kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
        kSecAccessControlPrivateKeyUsage | kSecAccessControlUserPresence, &error);
    if (!acl) return osError(error);
    NSDictionary *attributes = @{
        (__bridge id)kSecAttrKeyType: (__bridge id)kSecAttrKeyTypeECSECPrimeRandom,
        (__bridge id)kSecAttrKeySizeInBits: @256,
        (__bridge id)kSecAttrTokenID: (__bridge id)kSecAttrTokenIDSecureEnclave,
        (__bridge id)kSecUseDataProtectionKeychain: @YES,
        (__bridge id)kSecUseAuthenticationContext: context,
        (__bridge id)kSecPrivateKeyAttrs: @{
            (__bridge id)kSecAttrIsPermanent: @YES, (__bridge id)kSecAttrApplicationTag: tag,
            (__bridge id)kSecAttrAccessGroup: group, (__bridge id)kSecAttrAccessControl: (__bridge id)acl,
        },
    };
    CFRelease(acl);
    *result = SecKeyCreateRandomKey((__bridge CFDictionaryRef)attributes, &error);
    return *result ? errSecSuccess : osError(error);
}

static NSData *publicPoint(SecKeyRef key) {
    NSDictionary *attributes = CFBridgingRelease(SecKeyCopyAttributes(key));
    if (![attributes[(__bridge id)kSecAttrTokenID] isEqual:(__bridge id)kSecAttrTokenIDSecureEnclave] ||
        ![attributes[(__bridge id)kSecAttrKeyType] isEqual:(__bridge id)kSecAttrKeyTypeECSECPrimeRandom] ||
        ![attributes[(__bridge id)kSecAttrKeySizeInBits] isEqual:@256]) return nil;
    SecKeyRef publicKey = SecKeyCopyPublicKey(key);
    if (!publicKey) return nil;
    NSData *point = CFBridgingRelease(SecKeyCopyExternalRepresentation(publicKey, NULL));
    CFRelease(publicKey);
    return point.length == 65 && ((const uint8_t *)point.bytes)[0] == 4 ? point : nil;
}

@interface KagemushaDebitPromptContext : NSObject
@property(nonatomic, strong) LAContext *context;
@property(nonatomic) BOOL claimed;
@end
@implementation KagemushaDebitPromptContext
@end

static NSMutableDictionary<NSNumber *, KagemushaDebitPromptContext *> *contexts(void) {
    static NSMutableDictionary *active;
    static dispatch_once_t initialize;
    dispatch_once(&initialize, ^{ active = [NSMutableDictionary dictionary]; });
    return active;
}
#endif

__attribute__((visibility("hidden")))
int32_t kagemusha_apple_debit_key(const uint8_t slot[32], const uint8_t nonce[32],
                                  int32_t create, uint8_t output[65]) {
#if TARGET_OS_SIMULATOR || TARGET_OS_MACCATALYST
    return errSecNotAvailable;
#else
    @autoreleasepool {
        LAContext *context = [[LAContext alloc] init];
        context.interactionNotAllowed = YES;
        SecKeyRef key = NULL;
        OSStatus status = approvalKey(slot, nonce, create != 0, context, &key);
        if (status != errSecSuccess) { [context invalidate]; return status; }
        NSData *point = publicPoint(key);
        CFRelease(key);
        [context invalidate];
        if (!point) return errSecInvalidKeyRef;
        [point getBytes:output length:65];
        return errSecSuccess;
    }
#endif
}

__attribute__((visibility("hidden")))
int32_t kagemusha_apple_debit_open_prompt(uint64_t generation) {
#if TARGET_OS_SIMULATOR || TARGET_OS_MACCATALYST
    return errSecNotAvailable;
#else
    @autoreleasepool {
        NSMutableDictionary *active = contexts();
        @synchronized (active) {
            if (active[@(generation)]) return errSecDuplicateItem;
            KagemushaDebitPromptContext *entry = [[KagemushaDebitPromptContext alloc] init];
            entry.context = [[LAContext alloc] init];
            entry.context.touchIDAuthenticationAllowableReuseDuration = 0;
            active[@(generation)] = entry;
        }
        return errSecSuccess;
    }
#endif
}

__attribute__((visibility("hidden")))
void kagemusha_apple_debit_close_prompt(uint64_t generation) {
#if !TARGET_OS_SIMULATOR && !TARGET_OS_MACCATALYST
    @autoreleasepool {
        NSMutableDictionary *active = contexts();
        LAContext *context;
        @synchronized (active) {
            KagemushaDebitPromptContext *entry = active[@(generation)];
            context = entry.context;
            [active removeObjectForKey:@(generation)];
        }
        [context invalidate];
    }
#endif
}

#ifdef IROHA_SPARTAN_DEVICE_FIXTURE
// Declared for the Swift fixture in experiments/kagemusha-spartan/device/runtime/spartan_device_fixture.h.
__attribute__((visibility("hidden")))
int32_t kagemusha_apple_fixture_namespace_preimage_v1(uint8_t *output, size_t capacity, size_t *written) {
#if TARGET_OS_SIMULATOR || TARGET_OS_MACCATALYST
    return errSecNotAvailable;
#else
    @autoreleasepool {
        const uint8_t *preimage = fixturePreimage();
        if (!preimage) return errSecDecode;
        if (!output || !written || capacity < FIXTURE_PREIMAGE_BYTES) return errSecParam;
        memcpy(output, preimage, FIXTURE_PREIMAGE_BYTES);
        *written = FIXTURE_PREIMAGE_BYTES;
        return errSecSuccess;
    }
#endif
}
#endif

// Native supplies the pin from marker-selected custody, plus the frozen review and challenge.
__attribute__((visibility("hidden")))
int32_t kagemusha_apple_debit_sign(uint64_t generation, const uint8_t slot[32], const uint8_t nonce[32],
                                   const uint8_t pin[65], const uint8_t message[32], const uint8_t *terms,
                                   size_t termsLength, uint8_t output[72], size_t *written) {
#if TARGET_OS_SIMULATOR || TARGET_OS_MACCATALYST
    return errSecNotAvailable;
#else
    @autoreleasepool {
        if (NSThread.isMainThread) return errSecInteractionNotAllowed;
        NSString *reason = [[NSString alloc] initWithBytes:terms length:termsLength encoding:NSUTF8StringEncoding];
        if (!reason.length) return errSecParam;
        NSMutableDictionary *active = contexts();
        LAContext *context;
        @synchronized (active) {
            KagemushaDebitPromptContext *entry = active[@(generation)];
            if (!entry || entry.claimed) return errSecUserCanceled;
            entry.claimed = YES;
            context = entry.context;
        }
        context.localizedReason = reason;
        SecKeyRef key = NULL;
        OSStatus status = approvalKey(slot, nonce, NO, context, &key);
        if (status != errSecSuccess) { [context invalidate]; return status; }
        if (![publicPoint(key) isEqualToData:[NSData dataWithBytes:pin length:65]]) {
            CFRelease(key); [context invalidate]; return errSecInvalidKeyRef;
        }
        CFErrorRef error = NULL;
        NSData *signature = CFBridgingRelease(SecKeyCreateSignature(key,
            kSecKeyAlgorithmECDSASignatureMessageX962SHA256,
            (__bridge CFDataRef)[NSData dataWithBytes:message length:32], &error));
        CFRelease(key);
        [context invalidate];
        if (!signature) return osError(error);
        if (signature.length < 8 || signature.length > 72) return errSecAuthFailed;
        [signature getBytes:output length:signature.length];
        *written = signature.length;
        return errSecSuccess;
    }
#endif
}
