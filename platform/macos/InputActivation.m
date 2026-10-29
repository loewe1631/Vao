#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>

static id activationObserver;
static void (*activated)(int);
static void notifyElectron(NSRunningApplication *app) {
    if (!activated || !app || !app.isActive || !AXIsProcessTrusted()) return;
    NSString *identifier = app.bundleIdentifier.lowercaseString ?: @"";
    for (NSString *sensitive in @[@"password", @"keychain", @"bitwarden", @"lastpass", @"keepass", @"authenticator"]) {
        if ([identifier containsString:sensitive]) return;
    }
    NSURL *framework = [app.bundleURL URLByAppendingPathComponent:@"Contents/Frameworks/Electron Framework.framework"];
    if (framework && [NSFileManager.defaultManager fileExistsAtPath:framework.path]) activated(app.processIdentifier);
}
// Application identity only. No UI fields, titles, selection or clipboard are read.
void vao_install_input_activation(void (*callback)(int)) {
    activated = callback;
    if (activationObserver) return;
    activationObserver = [NSWorkspace.sharedWorkspace.notificationCenter
        addObserverForName:NSWorkspaceDidActivateApplicationNotification object:nil queue:NSOperationQueue.mainQueue
        usingBlock:^(NSNotification *notification) {
            notifyElectron(notification.userInfo[NSWorkspaceApplicationKey]);
        }];
    notifyElectron(NSWorkspace.sharedWorkspace.frontmostApplication);
}
