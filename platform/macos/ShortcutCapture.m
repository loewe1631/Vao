#import <AppKit/AppKit.h>
static id monitor;
static BOOL armed = NO;
static NSTimeInterval deadline;
static int (*receive)(unsigned char, unsigned short, unsigned long long, int);
void vao_install_local_capture(int (*callback)(unsigned char, unsigned short, unsigned long long, int)) {
    receive = callback;
    if (monitor) return;
    monitor = [NSEvent addLocalMonitorForEventsMatchingMask:(NSEventMaskFlagsChanged | NSEventMaskKeyDown | NSEventMaskKeyUp) handler:^NSEvent *(NSEvent *event) {
        if (!armed || NSProcessInfo.processInfo.systemUptime > deadline || !NSApp.isActive) return event;
        unsigned char kind = event.type == NSEventTypeFlagsChanged ? 0 : event.type == NSEventTypeKeyDown ? 1 : 2;
        return receive(kind, event.keyCode, event.modifierFlags, event.type == NSEventTypeKeyDown && event.isARepeat) ? nil : event;
    }];
}
void vao_set_local_capture(int enabled) {
    armed = enabled != 0;
    deadline = NSProcessInfo.processInfo.systemUptime + 12;
}
// Only called by the explicit isolated native test. Dispatches into this application's
// event loop; never posts a global keyboard event or reads another application's text.
void vao_probe_local_event(unsigned char kind, unsigned short key, unsigned long long flags) {
    NSEventType type = kind == 0 ? NSEventTypeFlagsChanged : kind == 1 ? NSEventTypeKeyDown : NSEventTypeKeyUp;
    NSEvent *event = [NSEvent keyEventWithType:type location:NSZeroPoint modifierFlags:flags timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:NSApp.keyWindow.windowNumber context:nil characters:@"" charactersIgnoringModifiers:@"" isARepeat:NO keyCode:key];
    [NSApp postEvent:event atStart:NO];
}
