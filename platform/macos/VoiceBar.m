#import <AppKit/AppKit.h>
#import <ApplicationServices/ApplicationServices.h>
#import <objc/runtime.h>
#import <stdatomic.h>

static char panelKey;
static _Atomic(uint32_t) overlayMouseEvents = 0;

// Create an actual nonactivating panel. Merely returning NO from
// canBecomeKeyWindow on a regular NSWindow still lets a mouse click activate its app.
@interface VaoVoicePanel : NSPanel
@end
@implementation VaoVoicePanel
- (BOOL)canBecomeKeyWindow { return NO; }
- (BOOL)canBecomeMainWindow { return NO; }
- (void)sendEvent:(NSEvent *)event {
    if (event.type == NSEventTypeLeftMouseDown || event.type == NSEventTypeRightMouseDown
        || event.type == NSEventTypeOtherMouseDown) {
        atomic_fetch_add(&overlayMouseEvents, 1);
    }
    [super sendEvent:event];
}
@end

static VaoVoicePanel *voicePanel(void *rawWindow) {
    return objc_getAssociatedObject((__bridge NSWindow *)rawWindow, &panelKey);
}
int vao_initialize_voice_bar(void *rawWindow) {
    NSWindow *owner = (__bridge NSWindow *)rawWindow;
    if (voicePanel(rawWindow)) return 1;
    NSView *content = owner.contentView;
    if (!content) return 0;
    VaoVoicePanel *panel = [[VaoVoicePanel alloc]
        initWithContentRect:owner.frame
        styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel
        backing:NSBackingStoreBuffered defer:NO];
    panel.releasedWhenClosed = NO;
    panel.opaque = NO;
    panel.backgroundColor = NSColor.clearColor;
    panel.hasShadow = NO;
    panel.level = NSStatusWindowLevel;
    panel.hidesOnDeactivate = NO;
    panel.becomesKeyOnlyIfNeeded = YES;
    panel.floatingPanel = YES;
    panel.animationBehavior = NSWindowAnimationBehaviorNone;
    panel.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces
        | NSWindowCollectionBehaviorFullScreenAuxiliary | NSWindowCollectionBehaviorIgnoresCycle;
    // Keep Tauri's window and WebView identities intact; only host its existing
    // content view in the panel. Never mutate a framework object's Objective-C class.
    [owner orderOut:nil];
    owner.contentView = [[NSView alloc] initWithFrame:content.frame];
    panel.contentView = content;
    content.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    objc_setAssociatedObject(owner, &panelKey, panel, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    return 1;
}
void vao_set_voice_bar_visible(void *rawWindow, int visible) {
    VaoVoicePanel *panel = voicePanel(rawWindow);
    if (visible) [panel orderFrontRegardless]; else [panel orderOut:nil];
}
int vao_voice_bar_visible(void *rawWindow) { return voicePanel(rawWindow).visible; }
uint32_t vao_voice_bar_mouse_events(void) { return atomic_load(&overlayMouseEvents); }

// Called only by the isolated --verify-ui harness. Post real WindowServer mouse
// events from a worker so the AppKit run loop can handle clicks and window drags.
int vao_probe_voice_bar_event(void *rawWindow, double x, double y, int kind) {
    VaoVoicePanel *panel = voicePanel(rawWindow);
    if (!panel.visible || x < 0 || y < 0 || x >= panel.frame.size.width
        || y >= panel.frame.size.height || kind < 0 || kind > 2) return 0;
    NSPoint screenPoint = [panel convertPointToScreen:NSMakePoint(x, panel.frame.size.height-y)];
    CGPoint point = CGPointMake(screenPoint.x, NSMaxY(NSScreen.screens.firstObject.frame)-screenPoint.y);
    dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
        CGPoint eventPoint = point;
        CGEventSourceRef source = CGEventSourceCreate(kCGEventSourceStateCombinedSessionState);
        CGEventRef saved = CGEventCreate(NULL);
        CGPoint old = CGEventGetLocation(saved);
        CFRelease(saved);
        CGEventRef move = CGEventCreateMouseEvent(source, kCGEventMouseMoved, eventPoint, kCGMouseButtonLeft);
        CGEventPost(kCGHIDEventTap, move); CFRelease(move);
        [NSThread sleepForTimeInterval:0.06];
        if (kind == 2) {
            CGEventRef scroll = CGEventCreateScrollWheelEvent(source, kCGScrollEventUnitPixel, 1, -160);
            CGEventSetLocation(scroll, eventPoint); CGEventPost(kCGHIDEventTap, scroll); CFRelease(scroll);
        } else {
            CGEventRef down = CGEventCreateMouseEvent(source, kCGEventLeftMouseDown, eventPoint, kCGMouseButtonLeft);
            CGEventPost(kCGHIDEventTap, down); CFRelease(down);
            [NSThread sleepForTimeInterval:0.14];
            if (kind == 1) {
                for (int i=1; i<=4; i++) {
                    CGPoint next = CGPointMake(eventPoint.x+15*i, eventPoint.y-10*i);
                    CGEventRef drag = CGEventCreateMouseEvent(source, kCGEventLeftMouseDragged, next, kCGMouseButtonLeft);
                    CGEventPost(kCGHIDEventTap, drag); CFRelease(drag);
                    [NSThread sleepForTimeInterval:0.05];
                }
                eventPoint.x += 60; eventPoint.y -= 40;
            }
            CGEventRef up = CGEventCreateMouseEvent(source, kCGEventLeftMouseUp, eventPoint, kCGMouseButtonLeft);
            CGEventPost(kCGHIDEventTap, up); CFRelease(up);
        }
        [NSThread sleepForTimeInterval:0.08];
        CGEventRef restore = CGEventCreateMouseEvent(source, kCGEventMouseMoved, old, kCGMouseButtonLeft);
        CGEventPost(kCGHIDEventTap, restore); CFRelease(restore);
        CFRelease(source);
    });
    return 1;
}
char *vao_inspect_voice_bar(void *rawWindow) {
    NSWindow *owner = (__bridge NSWindow *)rawWindow;
    VaoVoicePanel *panel = voicePanel(rawWindow);
    NSDictionary *state = @{@"visible":@(panel.visible), @"key":@(panel.keyWindow),
        @"main":@(panel.mainWindow), @"owner_visible":@(owner.visible),
        @"nonactivating_panel":@((BOOL)([panel isKindOfClass:NSPanel.class]
            && (panel.styleMask & NSWindowStyleMaskNonactivatingPanel) != 0)),
        @"x":@(panel.frame.origin.x), @"y":@(panel.frame.origin.y),
        @"width":@(panel.frame.size.width), @"height":@(panel.frame.size.height)};
    NSData *json = [NSJSONSerialization dataWithJSONObject:state options:0 error:nil];
    return strdup([[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding].UTF8String);
}

static CGDirectDisplayID displayID(NSScreen *screen) { return [screen.deviceDescription[@"NSScreenNumber"] unsignedIntValue]; }
static BOOL elementRect(AXUIElementRef element, CGRect *rect) {
    AXUIElementSetMessagingTimeout(element,0.15);
    CFTypeRef position=NULL,size=NULL;
    BOOL ok=AXUIElementCopyAttributeValue(element,kAXPositionAttribute,&position)==kAXErrorSuccess
        && AXUIElementCopyAttributeValue(element,kAXSizeAttribute,&size)==kAXErrorSuccess;
    CGPoint p;CGSize s;
    ok=ok&&position&&size&&CFGetTypeID(position)==AXValueGetTypeID()&&CFGetTypeID(size)==AXValueGetTypeID()
        &&AXValueGetValue(position,kAXValueCGPointType,&p)&&AXValueGetValue(size,kAXValueCGSizeType,&s);
    if(ok) { *rect=CGRectMake(p.x,p.y,s.width,s.height);ok=s.width>0&&s.height>0&&isfinite(p.x)&&isfinite(p.y); }
    if(position)CFRelease(position);if(size)CFRelease(size);return ok;
}
static NSInteger bestScreen(CGRect target, NSArray<NSValue *> *frames) {
    CGFloat best=0;NSInteger chosen=-1;
    for(NSUInteger i=0;i<frames.count;i++) {
        CGRect overlap=CGRectIntersection(target,frames[i].rectValue);
        CGFloat area=CGRectIsNull(overlap)?0:overlap.size.width*overlap.size.height;
        if(area>best){best=area;chosen=(NSInteger)i;}
    }
    return chosen;
}
// Logical AppKit points only. Keep the complete frame clear of the Dock/menu bar.
static NSRect fittedFrame(NSRect visible, NSSize size, NSPoint origin) {
    CGFloat inset=MIN(12,MIN(visible.size.width,visible.size.height)/4);
    NSRect safe=NSInsetRect(visible,inset,inset);
    size.width=MIN(size.width,safe.size.width);size.height=MIN(size.height,safe.size.height);
    origin.x=MAX(NSMinX(safe),MIN(origin.x,NSMaxX(safe)-size.width));
    origin.y=MAX(NSMinY(safe),MIN(origin.y,NSMaxY(safe)-size.height));
    return NSMakeRect(origin.x,origin.y,size.width,size.height);
}
static NSPoint barOrigin(NSRect visible, NSSize size) {
    return NSMakePoint(NSMidX(visible)-size.width/2,NSMinY(visible)+24);
}
static NSString *const positionKey=@"VaoVoiceBarPositions";
void vao_probe_voice_bar_positions(int begin) {
    static NSDictionary *savedPositions;
    if (begin) {
        savedPositions = [NSUserDefaults.standardUserDefaults dictionaryForKey:positionKey];
    } else {
        if (savedPositions) [NSUserDefaults.standardUserDefaults setObject:savedPositions forKey:positionKey];
        else [NSUserDefaults.standardUserDefaults removeObjectForKey:positionKey];
        savedPositions = nil;
    }
}
static NSRect frameForScreen(NSScreen *screen, NSInteger expanded) {
    NSRect visible=screen.visibleFrame;
    NSSize size=expanded==2?NSMakeSize(440,220):(expanded?NSMakeSize(520,360):NSMakeSize(380,148));
    NSPoint origin=barOrigin(visible,size);
    NSDictionary *saved=[[NSUserDefaults.standardUserDefaults dictionaryForKey:positionKey] objectForKey:[@(displayID(screen)) stringValue]];
    if([saved isKindOfClass:NSDictionary.class]) {
        NSNumber *x=saved[@"centerX"],*y=saved[@"bottomY"];
        if([x isKindOfClass:NSNumber.class]&&[y isKindOfClass:NSNumber.class]&&isfinite(x.doubleValue)&&isfinite(y.doubleValue)) {
            origin.x=NSMinX(visible)+MAX(0,MIN(1,x.doubleValue))*visible.size.width-size.width/2;
            origin.y=NSMinY(visible)+MAX(0,MIN(1,y.doubleValue))*visible.size.height;
        }
    }
    return fittedFrame(visible,size,origin);
}
static NSScreen *screenForFrame(NSRect frame) {
    NSArray<NSScreen *> *screens=NSScreen.screens;NSMutableArray<NSValue *> *frames=[NSMutableArray array];
    for(NSScreen *s in screens)[frames addObject:[NSValue valueWithRect:s.frame]];
    NSInteger index=bestScreen(frame,frames);
    return index>=0?screens[(NSUInteger)index]:(NSScreen.mainScreen?:screens.firstObject);
}
static void rememberFrame(NSWindow *window) {
    NSScreen *screen=screenForFrame(window.frame);if(!screen)return;
    NSRect visible=screen.visibleFrame,frame=fittedFrame(visible,window.frame.size,window.frame.origin);
    [window setFrame:frame display:NO];
    NSMutableDictionary *positions=[[NSUserDefaults.standardUserDefaults dictionaryForKey:positionKey] mutableCopy]?:[NSMutableDictionary dictionary];
    positions[[@(displayID(screen)) stringValue]]=@{@"centerX":@((NSMidX(frame)-NSMinX(visible))/visible.size.width),@"bottomY":@((NSMinY(frame)-NSMinY(visible))/visible.size.height)};
    [NSUserDefaults.standardUserDefaults setObject:positions forKey:positionKey];
}
unsigned int vao_capture_input_display(int pid) {
    NSArray<NSScreen *> *screens=NSScreen.screens;
    if(pid>0&&AXIsProcessTrusted()) {
        AXUIElementRef app=AXUIElementCreateApplication(pid);AXUIElementSetMessagingTimeout(app,0.15);
        CFTypeRef window=NULL,focused=NULL;CGRect windowRect=CGRectZero,focusRect=CGRectZero;
        BOOL hasWindow=AXUIElementCopyAttributeValue(app,kAXFocusedWindowAttribute,&window)==kAXErrorSuccess&&elementRect((AXUIElementRef)window,&windowRect);
        CGRect target=windowRect;BOOL hasTarget=hasWindow;
        if(AXUIElementCopyAttributeValue(app,kAXFocusedUIElementAttribute,&focused)==kAXErrorSuccess&&elementRect((AXUIElementRef)focused,&focusRect)) {
            // Prefer the focused control's visible area if a window spans two screens.
            CGRect clipped=hasWindow?CGRectIntersection(windowRect,focusRect):focusRect;
            if(!CGRectIsNull(clipped)&&!CGRectIsEmpty(clipped)){target=clipped;hasTarget=YES;}
        }
        if(window)CFRelease(window);if(focused)CFRelease(focused);CFRelease(app);
        if(hasTarget) {
            NSMutableArray<NSValue *> *frames=[NSMutableArray array];
            // AX and CGDisplayBounds share the top-left global coordinate system.
            for(NSScreen *screen in screens)[frames addObject:[NSValue valueWithRect:CGDisplayBounds(displayID(screen))]];
            NSInteger index=bestScreen(target,frames);
            if(index>=0)return displayID(screens[(NSUInteger)index]);
        }
    }
    NSPoint pointer=NSEvent.mouseLocation;
    for(NSScreen *screen in screens)if(NSPointInRect(pointer,screen.frame))return displayID(screen);
    return displayID(NSScreen.mainScreen ?: screens.firstObject);
}
// Sizing and placement must be atomic. Tao's set_inner_size is asynchronous on macOS.
void vao_place_voice_bar(void *rawWindow,unsigned int preferredDisplay,int expanded) {
    NSWindow *window=voicePanel(rawWindow);NSScreen *chosen=nil;
    for(NSScreen *screen in NSScreen.screens)if(displayID(screen)==preferredDisplay){chosen=screen;break;}
    if(!chosen){unsigned int fallback=vao_capture_input_display(0);for(NSScreen *screen in NSScreen.screens)if(displayID(screen)==fallback){chosen=screen;break;}}
    if(chosen)[window setFrame:frameForScreen(chosen,expanded) display:NO];
}
int vao_drag_voice_bar(void *rawWindow) {
    NSWindow *window=voicePanel(rawWindow);
    if(!(NSEvent.pressedMouseButtons&1))return 0;
    NSEvent *event=NSApp.currentEvent;
    if(event.type!=NSEventTypeLeftMouseDown&&event.type!=NSEventTypeLeftMouseDragged) {
        NSPoint point=[window convertPointFromScreen:NSEvent.mouseLocation];
        event=[NSEvent mouseEventWithType:NSEventTypeLeftMouseDown location:point modifierFlags:NSEvent.modifierFlags timestamp:NSProcessInfo.processInfo.systemUptime windowNumber:window.windowNumber context:nil eventNumber:0 clickCount:1 pressure:1];
    }
    window.movable=YES;
    [window performWindowDragWithEvent:event];
    rememberFrame(window);
    return 1;
}
unsigned int vao_test_display_geometry(void) {
    NSArray *horizontal=@[[NSValue valueWithRect:NSMakeRect(0,0,1728,1117)],[NSValue valueWithRect:NSMakeRect(1728,0,2560,1440)]];
    NSArray *vertical=@[[NSValue valueWithRect:NSMakeRect(0,0,1728,1117)],[NSValue valueWithRect:NSMakeRect(-200,-1440,2560,1440)]];
    unsigned int fail=0;
    if(bestScreen(CGRectMake(50,50,1400,900),horizontal)!=0)fail|=1;
    if(bestScreen(CGRectMake(1900,100,1400,900),horizontal)!=1)fail|=2;
    if(bestScreen(CGRectMake(100,-1200,1400,900),vertical)!=1)fail|=4;
    if(bestScreen(CGRectMake(1400,10,1000,800),horizontal)!=1)fail|=8;
    NSPoint p=barOrigin(NSMakeRect(-2560,24,2560,1416),NSMakeSize(232,60));
    if(p.x != -1396 || p.y != 48)fail|=16;
    NSArray *visible=@[[NSValue valueWithRect:NSMakeRect(0,65,2560,1345)],[NSValue valueWithRect:NSMakeRect(2560,0,2560,1440)],[NSValue valueWithRect:NSMakeRect(-1920,-1080,1920,1050)],[NSValue valueWithRect:NSMakeRect(0,0,320,200)]];
    for(NSValue *v in visible)for(NSValue *s in @[[NSValue valueWithSize:NSMakeSize(380,148)],[NSValue valueWithSize:NSMakeSize(520,360)],[NSValue valueWithSize:NSMakeSize(440,220)]]) {
        NSRect rect=v.rectValue;NSSize size=s.sizeValue;
        for(NSValue *o in @[[NSValue valueWithPoint:barOrigin(rect,size)],[NSValue valueWithPoint:NSMakePoint(NSMinX(rect)-100,NSMinY(rect)-500)],[NSValue valueWithPoint:NSMakePoint(NSMaxX(rect)+100,NSMaxY(rect)+500)]]) {
            if(!NSContainsRect(rect,fittedFrame(rect,size,o.pointValue)))fail|=32;
        }
    }
    return fail;
}
char *vao_probe_voice_bar(void *rawWindow) {
    NSWindow *window=voicePanel(rawWindow);NSRect old=window.frame;NSMutableArray *rows=[NSMutableArray array];
    NSUserDefaults *defaults=NSUserDefaults.standardUserDefaults;NSDictionary *saved=[defaults dictionaryForKey:positionKey];
    for(NSScreen *screen in NSScreen.screens){
        unsigned int identifier=displayID(screen);
        // Reproduce the exact small -> answer -> small transition on each monitor.
        for(NSNumber *expanded in @[@0,@1,@2,@0]) {
            vao_place_voice_bar(rawWindow,identifier,expanded.intValue);NSRect actual=window.frame;
            NSSize expected=expanded.intValue==2?NSMakeSize(440,220):(expanded.boolValue?NSMakeSize(520,360):NSMakeSize(380,148));
            [rows addObject:@{@"display_id":@(identifier),@"scale":@(screen.backingScaleFactor),@"expanded":expanded,@"contained":@(NSContainsRect(screen.visibleFrame,actual)),@"screen_matches":@(displayID(window.screen)==identifier),@"size_matches":@(NSEqualSizes(actual.size,expected)),@"x":@(actual.origin.x),@"y":@(actual.origin.y),@"width":@(actual.size.width),@"height":@(actual.size.height)}];
        }
        NSRect moved=window.frame;moved.origin=NSMakePoint(NSMidX(screen.visibleFrame)-moved.size.width/2,NSMinY(screen.visibleFrame)+120);
        [window setFrame:moved display:NO];rememberFrame(window);vao_place_voice_bar(rawWindow,identifier,1);
        NSRect expanded=window.frame;vao_place_voice_bar(rawWindow,identifier,0);
        BOOL restored=fabs(NSMinY(window.frame)-NSMinY(moved))<1&&fabs(NSMidX(window.frame)-NSMidX(moved))<1;
        [rows addObject:@{@"display_id":@(identifier),@"position_restored":@(restored),@"contained":@(NSContainsRect(screen.visibleFrame,expanded)&&NSContainsRect(screen.visibleFrame,window.frame)),@"screen_matches":@(displayID(window.screen)==identifier),@"size_matches":@YES}];
    }
    if(saved)[defaults setObject:saved forKey:positionKey];else[defaults removeObjectForKey:positionKey];
    [window setFrame:old display:NO];
    NSData *json=[NSJSONSerialization dataWithJSONObject:rows options:0 error:nil];
    return strdup([[NSString alloc] initWithData:json encoding:NSUTF8StringEncoding].UTF8String);
}
void vao_free_display_probe(char *value){free(value);}
