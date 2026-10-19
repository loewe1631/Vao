#import <AVFoundation/AVFoundation.h>
#import <ApplicationServices/ApplicationServices.h>
#import <CoreGraphics/CoreGraphics.h>
int vao_microphone_status(void) { return (int)[AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio]; }
void vao_request_microphone(void (*done)(int)) {
    [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio completionHandler:^(BOOL granted) { done(granted ? 1 : 0); }];
}
int vao_accessibility_status(void) { return AXIsProcessTrusted() ? 1 : 0; }
void vao_request_accessibility(void) {
    NSDictionary *options = @{(__bridge NSString *)kAXTrustedCheckOptionPrompt: @YES};
    AXIsProcessTrustedWithOptions((__bridge CFDictionaryRef)options);
}
int vao_input_status(void) { return CGPreflightListenEventAccess() ? 1 : 0; }
int vao_request_input(void) { return CGRequestListenEventAccess() ? 1 : 0; }

#import <Security/Security.h>
#import <LocalAuthentication/LocalAuthentication.h>
int vao_has_credential(const char *slot) {
    // Query metadata only. Never decrypt a password or open an authentication dialog
    // while rendering the UI, including after an ad-hoc signature update.
    LAContext *context = [[LAContext alloc] init];
    context.interactionNotAllowed = YES;
    NSDictionary *query = @{(__bridge NSString *)kSecClass: (__bridge NSString *)kSecClassGenericPassword,
        (__bridge NSString *)kSecAttrService: @"app.vaovao.desktop",
        (__bridge NSString *)kSecAttrAccount: [NSString stringWithUTF8String:slot],
        (__bridge NSString *)kSecMatchLimit: (__bridge NSString *)kSecMatchLimitOne,
        (__bridge NSString *)kSecReturnAttributes: @YES,
        (__bridge NSString *)kSecUseAuthenticationContext: context};
    CFTypeRef result = NULL;
    OSStatus status = SecItemCopyMatching((__bridge CFDictionaryRef)query, &result);
    if (result) CFRelease(result);
    return status == errSecSuccess ? 1 : status == errSecItemNotFound ? 0 : -1;
}

// Metadata only: no credential, selected text, audio or history is read here.
char *vao_permission_identity(void) {
    NSBundle *bundle = NSBundle.mainBundle;
    NSMutableDictionary *result = [@{
        @"bundle": bundle.bundleIdentifier ?: @"",
        @"application_path": bundle.bundlePath ?: @"",
        @"build": [bundle objectForInfoDictionaryKey:@"CFBundleVersion"] ?: @"",
        @"release_version": [bundle objectForInfoDictionaryKey:@"VaoReleaseVersion"] ?: @"",
        @"pid": @(NSProcessInfo.processInfo.processIdentifier)
    } mutableCopy];
    SecCodeRef code = NULL;
    OSStatus status = SecCodeCopySelf(kSecCSDefaultFlags, &code);
    if (status == errSecSuccess) {
        result[@"signature_status"] = @(SecCodeCheckValidity(code, kSecCSDefaultFlags, NULL));
        SecStaticCodeRef staticCode = NULL;
        if (SecCodeCopyStaticCode(code, kSecCSDefaultFlags, &staticCode) == errSecSuccess) {
            CFDictionaryRef info = NULL;
            if (SecCodeCopySigningInformation(staticCode, kSecCSSigningInformation, &info) == errSecSuccess) {
                NSDictionary *values = (__bridge NSDictionary *)info;
                result[@"signing_identifier"] = values[(__bridge NSString *)kSecCodeInfoIdentifier] ?: @"";
                result[@"adhoc"] = @(([values[(__bridge NSString *)kSecCodeInfoFlags] unsignedIntValue] & kSecCodeSignatureAdhoc) != 0);
                CFRelease(info);
            }
            SecRequirementRef requirement = NULL;
            if (SecCodeCopyDesignatedRequirement(staticCode, kSecCSDefaultFlags, &requirement) == errSecSuccess) {
                CFStringRef text = NULL;
                if (SecRequirementCopyString(requirement, kSecCSDefaultFlags, &text) == errSecSuccess) {
                    result[@"designated_requirement"] = (__bridge NSString *)text;
                    CFRelease(text);
                }
                CFRelease(requirement);
            }
            CFRelease(staticCode);
        }
        CFRelease(code);
    } else { result[@"signature_status"] = @(status); }
    NSData *data = [NSJSONSerialization dataWithJSONObject:result options:0 error:nil];
    NSString *text = data ? [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding] : @"{}";
    return strdup(text.UTF8String);
}
void vao_free_permission_identity(char *value) { free(value); }
