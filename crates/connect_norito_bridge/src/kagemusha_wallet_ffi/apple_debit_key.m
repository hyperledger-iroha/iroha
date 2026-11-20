// Source-owned Secure Enclave key creation, lookup and per-use signing for one admitted wallet.
#import <Foundation/Foundation.h>
#import <LocalAuthentication/LocalAuthentication.h>
#import <Security/Security.h>
#import <TargetConditionals.h>

#if !TARGET_OS_SIMULATOR && !TARGET_OS_MACCATALYST
static NSData *keyTag(NSString *prefix, const uint8_t slot[32], const uint8_t *nonce) {
    NSMutableString *name = [prefix mutableCopy];
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
    LAContext *probe = [[LAContext alloc] init];
    probe.interactionNotAllowed = YES;
    NSDictionary *payment = nil;
    OSStatus status = uniqueEnclaveItem(keyTag(@"kgm-w1-", slot, NULL), nil, probe, NO, &payment);
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
    NSData *tag = keyTag(@"kgm-debit1-", slot, nonce);
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
