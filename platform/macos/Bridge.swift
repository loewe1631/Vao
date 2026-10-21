// Vao macOS adapter. Invoked only by explicit desktop actions.
import AppKit
import ApplicationServices
import CryptoKit
import Carbon
import UniformTypeIdentifiers
var probeClipboard: [[(NSPasteboard.PasteboardType,Data)]]?
struct CapturedTarget {
    let element:AXUIElement
    let window:AXUIElement?
    let composer:Bool
    let windowPaste:Bool
    let mouseCount:UInt32
    let keyCount:UInt32
    let snapshot:FieldSnapshot?
    let bundle:String
}
// Only the explicitly captured input is fingerprinted, in helper memory. This
// state is neither persisted nor sent to model services.
struct FieldSnapshot {
    let valueDigest:Data
    let range:CFRange?
    let marker:CFTypeRef?
    static func read(_ element:AXUIElement)->FieldSnapshot? {
        guard isEditable(element),let value=attribute(element,kAXValueAttribute) as? String,
              value.utf8.count<=1024*1024 else{return nil}
        var range:CFRange?
        if let raw=attribute(element,kAXSelectedTextRangeAttribute),CFGetTypeID(raw)==AXValueGetTypeID() {
            var r=CFRange();if AXValueGetValue(unsafeBitCast(raw,to:AXValue.self),.cfRange,&r){range=r}
        }
        let marker=range == nil ? attribute(element,"AXSelectedTextMarkerRange") : nil
        guard range != nil || marker != nil else{return nil}
        return FieldSnapshot(valueDigest:Data(SHA256.hash(data:Data(value.utf8))),range:range,marker:marker)
    }
    func matches(_ element:AXUIElement)->Bool {
        guard let current=FieldSnapshot.read(element),current.valueDigest==valueDigest else{return false}
        if let a=range,let b=current.range{return a.location==b.location && a.length==b.length}
        if let a=marker,let b=current.marker{return CFEqual(a,b)}
        return false
    }
}
var capturedElements: [String: CapturedTarget] = [:]
// One bounded worker uses the original AX element. Replies travel over the
// dedicated stderr pipe, so AX reads never hold the target/delivery RPC queue.
let contextLock=NSLock()
var contextBusy=false
func inputContext(_ original:CapturedTarget,_ expanded:Bool)->[String:Any]? {
    guard AXIsProcessTrusted(),!original.windowPaste,let snapshot=original.snapshot,
          let saved=snapshot.range,saved.length==0 else{return nil}
    let element=original.element;AXUIElementSetMessagingTimeout(element,0.04)
    defer{AXUIElementSetMessagingTimeout(element,0.3)}
    let deadline=Date(timeIntervalSinceNow:0.45)
    guard isEditable(element),Date()<deadline,
          let value=attribute(element,kAXValueAttribute) as? String,
          value.utf8.count<=1024*1024,Date()<deadline,
          Data(SHA256.hash(data:Data(value.utf8)))==snapshot.valueDigest,
          let raw=attribute(element,kAXSelectedTextRangeAttribute),CFGetTypeID(raw)==AXValueGetTypeID() else{return nil}
    var current=CFRange();guard AXValueGetValue(unsafeBitCast(raw,to:AXValue.self),.cfRange,&current),
          current.location==saved.location,current.length==0,Date()<deadline,
          let cursor=contextCursor(value,current.location) else{return nil}
    var result=croppedContext(value,cursor)
    if expanded,Date()<deadline,!sensitiveApplication(original.bundle),let window=original.window {
        let extra=nearbyContext(window,original.element,deadline)
        for (key,value) in extra{result[key]=value}
    }
    guard Date()<deadline,snapshot.matches(element) else{return nil}
    return result
}
func sensitiveApplication(_ bundle:String)->Bool {
    ["password", "keychain", "bitwarden", "lastpass", "keepass", "authenticator"].contains{bundle.lowercased().contains($0)}
}
func contextCursor(_ value:String,_ offset:Int)->String.Index? {
    guard offset>=0,let u=value.utf16.index(value.utf16.startIndex,offsetBy:offset,limitedBy:value.utf16.endIndex) else{return nil}
    return String.Index(u,within:value)
}
func croppedContext(_ value:String,_ cursor:String.Index)->[String:Any] {
    var before=String(value[..<cursor].suffix(500))
    var after=String(value[cursor...].prefix(500))
    while before.utf8.count+after.utf8.count>8192 {
        if before.utf8.count>after.utf8.count {before.removeFirst()} else {after.removeLast()}
    }
    return ["version":1,"before_cursor":before,"after_cursor":after]
}
func startContext(_ token:String,_ original:CapturedTarget,_ expanded:Bool)->Bool {
    contextLock.lock();if contextBusy{contextLock.unlock();return false};contextBusy=true;contextLock.unlock()
    DispatchQueue.global(qos:.utility).async {
        defer{contextLock.lock();contextBusy=false;contextLock.unlock()}
        let context=inputContext(original,expanded)
        let reply:[String:Any]=["context_token":token,"context":context as Any? ?? NSNull()]
        if var data=try? JSONSerialization.data(withJSONObject:reply) {data.append(10);FileHandle.standardError.write(data)}
    }
    return true
}
// Read only bounded visible static text from the originally captured window.
func nearbyContext(_ window:AXUIElement,_ input:AXUIElement,_ deadline:Date)->[String:Any] {
    AXUIElementSetMessagingTimeout(window,0.025)
    defer{AXUIElementSetMessagingTimeout(window,0.3)}
    var out:[String:Any]=[:]
    if Date()<deadline,let title=attribute(window,kAXTitleAttribute) as? String {out["window_title"]=String(String.UnicodeScalarView(title.unicodeScalars.prefix(120)))}
    if Date()<deadline,let raw=attribute(window,kAXDocumentAttribute) as? String,let url=URL(string:raw),["https","http"].contains(url.scheme ?? ""),let host=url.host {out["source_domain"]=String(host.prefix(253))}
    guard Date()<deadline,let windowRect=elementRect(window) else{return out}
    var queue:[(AXUIElement,Int)]=[(window,0)],scanned=0,lines:[String]=[],remaining=1200
    while !queue.isEmpty,scanned<60,remaining>0,Date()<deadline {
        let (element,depth)=queue.removeFirst();scanned+=1
        if CFEqual(element,input){continue}
        AXUIElementSetMessagingTimeout(element,0.015)
        defer{AXUIElementSetMessagingTimeout(element,0.3)}
        if attribute(element,"AXHidden") as? Bool == true {continue}
        let role=attribute(element,kAXRoleAttribute) as? String ?? ""
        if (attribute(element,kAXSubroleAttribute) as? String)=="AXSecureTextField" || ["AXSecureTextField","AXTextField","AXTextArea","AXComboBox"].contains(role) {continue}
        if role==kAXStaticTextRole as String,Date()<deadline,let rect=elementRect(element),rect.width>0,rect.height>0,rect.intersects(windowRect),let text=attribute(element,kAXValueAttribute) as? String,!text.trimmingCharacters(in:.whitespacesAndNewlines).isEmpty {
            let clipped=String(String.UnicodeScalarView(text.unicodeScalars.prefix(min(400,remaining))));if !lines.contains(clipped){lines.append(clipped);remaining-=clipped.unicodeScalars.count+1}
        }
        if depth<5,Date()<deadline,let children=(attribute(element,"AXVisibleChildren") ?? attribute(element,kAXChildrenAttribute)) as? [AXUIElement] {queue.append(contentsOf:children.prefix(max(0,60-scanned-queue.count)).map{($0,depth+1)})}
    }
    out["nearby_text"]=lines.joined(separator:"\n");return out
}
func pasteKeyCode()->CGKeyCode? {
    for source in [TISCopyCurrentKeyboardLayoutInputSource()?.takeRetainedValue(),TISCopyCurrentASCIICapableKeyboardLayoutInputSource()?.takeRetainedValue()] {
        guard let source,let pointer=TISGetInputSourceProperty(source,kTISPropertyUnicodeKeyLayoutData) else{continue}
        let data=unsafeBitCast(pointer,to:CFData.self)
        guard let bytes=CFDataGetBytePtr(data) else{continue}
        let layout=UnsafeRawPointer(bytes).assumingMemoryBound(to:UCKeyboardLayout.self)
        for key in UInt16(0)..<UInt16(128) {
            var dead:UInt32=0,actual=0,buffer=[UniChar](repeating:0,count:4)
            let status=UCKeyTranslate(layout,key,UInt16(kUCKeyActionDown),UInt32(cmdKey >> 8),UInt32(LMGetKbdType()),OptionBits(kUCKeyTranslateNoDeadKeysMask),&dead,4,&actual,&buffer)
            if status==noErr,actual==1,(buffer[0]==118 || buffer[0]==86){return CGKeyCode(key)}
        }
    }
    return nil
}

func mouseCount()->UInt32 {
    return CGEventSource.counterForEventType(.combinedSessionState,eventType:.leftMouseDown)
        &+ CGEventSource.counterForEventType(.combinedSessionState,eventType:.rightMouseDown)
        &+ CGEventSource.counterForEventType(.combinedSessionState,eventType:.otherMouseDown)
}
func keyCount()->UInt32 {CGEventSource.counterForEventType(.combinedSessionState,eventType:.keyDown)}
func axElement(_ raw:CFTypeRef?)->AXUIElement? {
    guard let raw,CFGetTypeID(raw)==AXUIElementGetTypeID() else{return nil}
    return unsafeBitCast(raw,to:AXUIElement.self)
}
func focusedElement(_ process:AXUIElement,_ pid:pid_t)->AXUIElement? {
    if let focused=axElement(attribute(process,kAXFocusedUIElementAttribute)){return focused}
    let system=AXUIElementCreateSystemWide();AXUIElementSetMessagingTimeout(system,0.15)
    if let focused=axElement(attribute(system,kAXFocusedUIElementAttribute)) {
        var owner:pid_t=0
        if AXUIElementGetPid(focused,&owner) == .success,owner==pid{return focused}
    }
    return nil
}
@discardableResult
func enableAccessibility(_ process:AXUIElement)->[String:Int32] {
    var result:[String:Int32]=[:]
    for name in ["AXManualAccessibility","AXEnhancedUserInterface"] {
        result[name]=AXUIElementSetAttributeValue(process,name as CFString,kCFBooleanTrue).rawValue
    }
    return result
}
func elementRect(_ element:AXUIElement)->CGRect? {
    guard let p=attribute(element,kAXPositionAttribute),let s=attribute(element,kAXSizeAttribute),
        CFGetTypeID(p)==AXValueGetTypeID(),CFGetTypeID(s)==AXValueGetTypeID() else{return nil}
    var point=CGPoint.zero;var size=CGSize.zero
    guard AXValueGetValue(unsafeBitCast(p,to:AXValue.self),.cgPoint,&point),AXValueGetValue(unsafeBitCast(s,to:AXValue.self),.cgSize,&size),size.width>0,size.height>0 else{return nil}
    return CGRect(origin:point,size:size)
}
func isEditable(_ element:AXUIElement)->Bool {
    let role=attribute(element,kAXRoleAttribute) as? String ?? ""
    let subrole=attribute(element,kAXSubroleAttribute) as? String ?? ""
    guard ["AXTextArea","AXTextField","AXComboBox"].contains(role),subrole != "AXSecureTextField" else{return false}
    return attribute(element,kAXEnabledAttribute) as? Bool != false
}
// WeChat's conversation can be active while its composer has never been clicked.
// Only accept one visible lower-window text area. Search boxes and ambiguous fields
// are deliberately excluded. No message value is read while discovering controls.
func findComposer(_ window:AXUIElement)->AXUIElement? {
    guard let bounds=elementRect(window) else{return nil}
    var queue=[window];var candidates:[AXUIElement]=[];var scanned=0
    let deadline=Date(timeIntervalSinceNow:0.7)
    while !queue.isEmpty&&scanned<500&&Date()<deadline {
        let element=queue.removeFirst();scanned+=1;AXUIElementSetMessagingTimeout(element,0.08)
        let role=attribute(element,kAXRoleAttribute) as? String ?? ""
        if role=="AXTextArea",isEditable(element),let rect=elementRect(element),
           bounds.intersects(rect),rect.midY>bounds.minY+bounds.height*0.5,
           rect.width>bounds.width*0.25 {candidates.append(element)}
        if let children=attribute(element,kAXChildrenAttribute) as? [AXUIElement] {queue.append(contentsOf:children)}
    }
    return queue.isEmpty && candidates.count==1 ? candidates[0] : nil
}

func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    return AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success ? value : nil
}
// Query the AX system focus on every request. A stdin-driven daemon does not run
// NSWorkspace's notification loop, so its cached frontmostApplication can be stale.
func frontmost() -> NSRunningApplication? {
    let system=AXUIElementCreateSystemWide();AXUIElementSetMessagingTimeout(system,0.3)
    if let raw=attribute(system,kAXFocusedApplicationAttribute) {
        let element=unsafeBitCast(raw,to:AXUIElement.self);var pid:pid_t=0
        if AXUIElementGetPid(element,&pid) == .success {return NSRunningApplication(processIdentifier:pid)}
    }
    // No AX trust: refresh workspace notifications before its fallback query.
    RunLoop.current.run(until:Date(timeIntervalSinceNow:0.005))
    return NSWorkspace.shared.frontmostApplication
}
func selectedText(_ element:AXUIElement) -> String? {
    if let text=attribute(element,kAXSelectedTextAttribute) as? String,!text.isEmpty {return text}
    // Chromium/WebKit often expose a text-marker selection instead of AXSelectedText.
    if let range=attribute(element,"AXSelectedTextMarkerRange") {
        var raw:CFTypeRef?
        if AXUIElementCopyParameterizedAttributeValue(element,"AXAttributedStringForTextMarkerRange" as CFString,range,&raw) == .success,
           let text=raw as? NSAttributedString,!text.string.isEmpty {return text.string}
    }
    return nil
}
func copyMenu(_ process:AXUIElement) -> AXUIElement? {
    guard let raw=attribute(process,kAXMenuBarAttribute) else{return nil}
    var queue=[unsafeBitCast(raw,to:AXUIElement.self)];var scanned=0
    let deadline=Date(timeIntervalSinceNow:0.5)
    while !queue.isEmpty&&scanned<240&&Date()<deadline {
        let element=queue.removeFirst();scanned+=1;AXUIElementSetMessagingTimeout(element,0.1)
        if (attribute(element,"AXMenuItemCmdChar") as? String)?.lowercased()=="c",
           (attribute(element,"AXMenuItemCmdModifiers") as? Int ?? 0)==0,
           attribute(element,kAXEnabledAttribute) as? Bool == true {return element}
        if let children=attribute(element,kAXChildrenAttribute) as? [AXUIElement] {queue.append(contentsOf:children)}
    }
    return nil
}
// Fallback only for a known positive selection, or unknown range with an enabled
// native Copy menu. Never copy a whole editor line at a known zero-length cursor.
func copySelection(_ process:AXUIElement,_ pid:pid_t,_ length:Int?) -> String? {
    guard length != 0,let menu=copyMenu(process),frontmost()?.processIdentifier==pid else{return nil}
    let pb=NSPasteboard.general
    let saved=pb.pasteboardItems?.map{item -> [(NSPasteboard.PasteboardType,Data)] in item.types.compactMap{type in item.data(forType:type).map{(type,$0)}}} ?? []
    guard saved.flatMap({$0}).reduce(0,{$0+$1.1.count}) <= 10*1024*1024 else{return nil}
    let before=pb.changeCount
    guard AXUIElementPerformAction(menu,kAXPressAction as CFString) == .success else{return nil}
    let deadline=Date(timeIntervalSinceNow:0.65)
    while pb.changeCount==before&&Date()<deadline {Thread.sleep(forTimeInterval:0.015)}
    guard pb.changeCount != before else{return nil} // Do not read stale clipboard text.
    let copiedCount=pb.changeCount;let text=pb.string(forType:.string)
    let sameApp=frontmost()?.processIdentifier==pid
    if pb.changeCount==copiedCount {restore(saved)}
    guard sameApp else{return nil}
    return text
}
func target(_ readSelection: Bool, capture: Bool = false, allowCopy:Bool = true, discover:Bool = true) -> [String: Any] {
    guard let app=frontmost() else{return [:]}
    var out:[String:Any]=["pid":Int(app.processIdentifier),"bundle":app.bundleIdentifier ?? "","name":app.localizedName ?? "","target_mode":"unavailable"]
    guard AXIsProcessTrusted() else {out["capture_reason"]="accessibility_unavailable";return out}
    let process=AXUIElementCreateApplication(app.processIdentifier);AXUIElementSetMessagingTimeout(process,0.3)
    let isChat=app.bundleIdentifier=="com.tencent.xinWeChat" || ["app.vaovao.fixturecomposer","app.vaovao.fixtureambiguous","app.vaovao.fixtureopaque"].contains(app.bundleIdentifier ?? "")
    var focused=focusedElement(process,app.processIdentifier)
    if isChat&&(focused==nil || !isEditable(focused!)){enableAccessibility(process);focused=focusedElement(process,app.processIdentifier)}
    let fixtureOpaque=CommandLine.arguments.contains("--verify-selection") && app.bundleIdentifier=="app.vaovao.fixtureopaque"
    if fixtureOpaque{focused=nil}
    let window=axElement(attribute(process,kAXFocusedWindowAttribute))
    // Preserve a real selected message before looking for a destination composer.
    var text=readSelection ? focused.flatMap{selectedText($0)} : nil
    var selectionSource="accessibility"
    if readSelection,text==nil,allowCopy {
        var length:Int?
        if let f=focused,let raw=attribute(f,kAXSelectedTextRangeAttribute),CFGetTypeID(raw)==AXValueGetTypeID(){var range=CFRange();if AXValueGetValue(unsafeBitCast(raw,to:AXValue.self),.cfRange,&range){length=range.length}}
        text=copySelection(process,app.processIdentifier,length);selectionSource="copy"
    }
    var composer=isChat && text==nil && focused.map(isEditable)==true
    if isChat,discover,!fixtureOpaque,text==nil,focused==nil || !isEditable(focused!),let window,let input=findComposer(window){focused=input;composer=true}
    // Some native WeChat builds expose only the window, while retaining a valid
    // keyboard recipient internally. Preserve that recipient; never guess click coordinates.
    let windowPaste=text==nil && focused==nil && window != nil && (app.bundleIdentifier=="com.tencent.xinWeChat" || app.bundleIdentifier=="app.vaovao.fixtureopaque")
    if windowPaste {focused=window}
    if isChat,text==nil,!composer,!windowPaste{focused=nil}
    guard let focused else {
        if let text,!text.isEmpty {if text.utf8.count<=20000{out["selection"]=text;out["selection_source"]=selectionSource}else{out["selection_error"]=true}}
        out["capture_reason"]="focused_element_unavailable";return out
    }
    out["target_mode"]=windowPaste ? "window_paste" : (composer ? "composer" : "focused")
    if capture {
        if capturedElements.count>=4{capturedElements.removeAll()}
        let token=UUID().uuidString
        capturedElements[token]=CapturedTarget(element:focused,window:window,composer:composer,windowPaste:windowPaste,mouseCount:mouseCount(),keyCount:keyCount(),snapshot:FieldSnapshot.read(focused),bundle:app.bundleIdentifier ?? "");out["captureToken"]=token
    }
    out["role"]=attribute(focused,kAXRoleAttribute) as? String ?? ""
    out["identifier"]=attribute(focused,kAXIdentifierAttribute) as? String ?? ""
    if let window{out["window"]=attribute(window,"AXWindowNumber") as? Int ?? 0}
    if let r=attribute(focused,kAXSelectedTextRangeAttribute),CFGetTypeID(r)==AXValueGetTypeID(){var range=CFRange();if AXValueGetValue(unsafeBitCast(r,to:AXValue.self),.cfRange,&range){out["offset"]=range.location;out["length"]=range.length}}
    if readSelection,!composer {
        if let text,!text.isEmpty {if text.utf8.count<=20000{out["selection"]=text;out["selection_source"]=selectionSource}else{out["selection_error"]=true}}
        else if (out["length"] as? Int ?? 0)>0{out["selection_error"]=true}
    }
    return out
}
func sameTarget(_ a:[String:Any],_ b:[String:Any]) -> Bool {
    guard let pid=a["pid"] as? Int, pid == b["pid"] as? Int,
          let role=a["role"] as? String, !role.isEmpty, role == b["role"] as? String,
          (a["offset"] as? Int) == (b["offset"] as? Int),
          (a["length"] as? Int) == (b["length"] as? Int) else {return false}
    return (a["identifier"] as? String ?? "") == (b["identifier"] as? String ?? "") && (a["window"] as? Int ?? 0) == (b["window"] as? Int ?? 0)
}
func liveLockedWindow(_ original:CapturedTarget,_ pid:pid_t)->AXUIElement? {
    guard let app=NSRunningApplication(processIdentifier:pid),!app.isTerminated,
          let window=original.window else{return nil}
    var owner:pid_t=0
    guard AXUIElementGetPid(original.element,&owner) == .success,owner==pid else{return nil}
    let process=AXUIElementCreateApplication(pid);AXUIElementSetMessagingTimeout(process,0.3)
    guard let windows=attribute(process,kAXWindowsAttribute) as? [AXUIElement],
          windows.contains(where:{CFEqual($0,window)}) else{return nil}
    return window
}
func waitFor(_ condition:()->Bool)->Bool {
    let deadline=Date(timeIntervalSinceNow:1)
    repeat {
        if condition(){return true}
        RunLoop.current.run(until:Date(timeIntervalSinceNow:0.015))
    } while Date()<deadline
    return condition()
}
func activateLockedWindow(_ original:CapturedTarget,_ pid:pid_t)->String? {
    guard let window=liveLockedWindow(original,pid),let app=NSRunningApplication(processIdentifier:pid)
        else{return "原窗口已关闭，结果已保留 / Original window unavailable"}
    let mouse=mouseCount(),keys=keyCount()
    if attribute(window,kAXMinimizedAttribute) as? Bool == true {
        _=AXUIElementSetAttributeValue(window,kAXMinimizedAttribute as CFString,kCFBooleanFalse)
    }
    _=AXUIElementPerformAction(window,kAXRaiseAction as CFString)
    app.activate(options:[.activateIgnoringOtherApps])
    let process=AXUIElementCreateApplication(pid);AXUIElementSetMessagingTimeout(process,0.3)
    _=AXUIElementSetAttributeValue(window,kAXMainAttribute as CFString,kCFBooleanTrue)
    _=AXUIElementPerformAction(window,kAXRaiseAction as CFString)
    guard waitFor({frontmost()?.processIdentifier==pid && axElement(attribute(process,kAXFocusedWindowAttribute)).map{CFEqual($0,window)} == true}),
          mouseCount()==mouse,keyCount()==keys else {
        return "无法恢复原窗口，结果已保留 / Could not restore the original window"
    }
    return nil
}
func restoreLockedInput(_ original:CapturedTarget,_ pid:pid_t)->String? {
    guard liveLockedWindow(original,pid) != nil else{return "原窗口已关闭，结果已保留 / Original window unavailable"}
    guard let snapshot=original.snapshot,snapshot.matches(original.element) else {
        return "原输入内容或光标已变化，结果已保留 / Original input or caret changed"
    }
    guard let window=original.window,
          let fieldWindow=axElement(attribute(original.element,kAXWindowAttribute)),CFEqual(fieldWindow,window) else {
        return "原输入框已失效，结果已保留 / Original input field unavailable"
    }
    let mouse=mouseCount(),keys=keyCount()
    if let error=activateLockedWindow(original,pid){return error}
    _=AXUIElementSetAttributeValue(original.element,kAXFocusedAttribute as CFString,kCFBooleanTrue)
    let process=AXUIElementCreateApplication(pid);AXUIElementSetMessagingTimeout(process,0.3)
    guard waitFor({focusedElement(process,pid).map{CFEqual($0,original.element)} == true}),
          snapshot.matches(original.element),liveLockedWindow(original,pid) != nil,
          mouseCount()==mouse,keyCount()==keys else {
        return "无法恢复原输入位置，结果已保留 / Could not restore the original input"
    }
    return nil
}
func run(_ request:[String:Any]) -> [String:Any] {
    switch request["action"] as? String ?? "" {
    case "self_test":
        let a:[String:Any] = ["pid":123,"role":"AXTextArea","identifier":"synthetic","window":10,"offset":3,"length":0]
        var b=a; b["pid"]=124
        var c=a; c["offset"]=4
        return ["same":sameTarget(a,a),"differentPIDRejected":!sameTarget(a,b),"differentCursorRejected":!sameTarget(a,c),"missingTargetRejected":!sameTarget([:],[:])]
    case "probe_clipboard":
        guard CommandLine.arguments.contains("--verify-selection") else{return ["error":"Diagnostic only"]}
        let pb=NSPasteboard.general
        switch request["mode"] as? String ?? "" {
        case "begin":
            probeClipboard=pb.pasteboardItems?.map{item in item.types.compactMap{type in item.data(forType:type).map{(type,$0)}}} ?? []
            pb.clearContents();pb.setString("synthetic-sentinel",forType:.string);pb.setData(Data([1,2,3]),forType:NSPasteboard.PasteboardType("app.vaovao.synthetic"))
            return ["ok":true]
        case "check":return ["ok":pb.string(forType:.string)=="synthetic-sentinel"&&pb.data(forType:NSPasteboard.PasteboardType("app.vaovao.synthetic"))==Data([1,2,3])]
        case "restore":if let saved=probeClipboard {restore(saved);probeClipboard=nil};return ["ok":true]
        default:return ["error":"Invalid probe"]
        }
    case "probe_copy_selection":
        guard CommandLine.arguments.contains("--verify-selection") else{return ["error":"Diagnostic only"]}
        let current=target(false);guard let pid=current["pid"] as? Int else{return ["error":"No fixture"]}
        let process=AXUIElementCreateApplication(pid_t(pid));AXUIElementSetMessagingTimeout(process,0.3)
        let text=copySelection(process,pid_t(pid),current["length"] as? Int)
        return ["text":text ?? "","pid":pid]
    case "inspect_app":
        guard CommandLine.arguments.contains("--verify-selection"),let pid=request["pid"] as? Int else{return ["error":"Diagnostic only"]}
        let process=AXUIElementCreateApplication(pid_t(pid));AXUIElementSetMessagingTimeout(process,0.2)
        let before=focusedElement(process,pid_t(pid));enableAccessibility(process)
        let enabled=enableAccessibility(process);Thread.sleep(forTimeInterval:0.15)
        let after=focusedElement(process,pid_t(pid));let window=axElement(attribute(process,kAXFocusedWindowAttribute));let composer=window.flatMap{findComposer($0)}
        var nodes:[[String:Any]]=[]
        if let window {var queue=[window];while !queue.isEmpty&&nodes.count<100 {let e=queue.removeFirst();AXUIElementSetMessagingTimeout(e,0.05);let role=attribute(e,kAXRoleAttribute) as? String ?? "";var row:[String:Any]=["role":role];if let r=elementRect(e){row["rect"]=[r.minX,r.minY,r.width,r.height]};if let ch=attribute(e,kAXChildrenAttribute) as? [AXUIElement]{queue.append(contentsOf:ch);row["children"]=ch.count};nodes.append(row)}}
        let mode=frontmost()?.processIdentifier==pid_t(pid) ? (target(false)["target_mode"] as? String ?? "") : ""
        return ["target_mode":mode,"enable_results":enabled,"nodes":nodes,"trusted":AXIsProcessTrusted(),"before_role":before.flatMap{attribute($0,kAXRoleAttribute) as? String} ?? "","after_role":after.flatMap{attribute($0,kAXRoleAttribute) as? String} ?? "","window":window != nil,"composer":composer != nil]
    case "probe_locked_interaction":
        guard CommandLine.arguments.contains("--verify-selection"),let app=frontmost(),app.bundleIdentifier=="app.vaovao.fixturebrowser" else{return ["error":"Synthetic browser only"]}
        let process=AXUIElementCreateApplication(app.processIdentifier)
        guard let focused=focusedElement(process,app.processIdentifier),let rect=elementRect(focused),isEditable(focused) else{return ["error":"Missing synthetic input"]}
        let mouse=mouseCount(),keys=keyCount(),old=CGEvent(source:nil)?.location
        let point=CGPoint(x:rect.midX,y:rect.midY),source=CGEventSource(stateID:.combinedSessionState)
        CGEvent(mouseEventSource:source,mouseType:.leftMouseDown,mouseCursorPosition:point,mouseButton:.left)?.post(tap:.cghidEventTap)
        CGEvent(mouseEventSource:source,mouseType:.leftMouseUp,mouseCursorPosition:point,mouseButton:.left)?.post(tap:.cghidEventTap)
        CGEvent(keyboardEventSource:source,virtualKey:0,keyDown:true)?.post(tap:.cghidEventTap)
        CGEvent(keyboardEventSource:source,virtualKey:0,keyDown:false)?.post(tap:.cghidEventTap)
        Thread.sleep(forTimeInterval:0.15)
        if let old{CGWarpMouseCursorPosition(old)}
        return ["mouse":mouseCount() &- mouse,"keys":keyCount() &- keys]
    case "probe_fixture":
        guard CommandLine.arguments.contains("--verify-selection"),let app=frontmost(),["app.vaovao.fixturecomposer","app.vaovao.fixtureambiguous","app.vaovao.fixtureopaque"].contains(app.bundleIdentifier ?? "") else{return ["error":"Fixture only"]}
        let process=AXUIElementCreateApplication(app.processIdentifier)
        guard let window=axElement(attribute(process,kAXFocusedWindowAttribute)) else{return ["error":"Missing fixture window"]}
        let mode=request["mode"] as? String ?? ""
        if mode=="read",let composer=findComposer(window){return ["text":attribute(composer,kAXValueAttribute) as? String ?? ""]}
        if mode=="fn_chord_count" {
            let before=keyCount();let source=CGEventSource(stateID:.combinedSessionState)
            let f=CGEvent(keyboardEventSource:source,virtualKey:63,keyDown:true);f?.type = .flagsChanged;f?.flags = .maskSecondaryFn;f?.post(tap:.cghidEventTap)
            let d=CGEvent(keyboardEventSource:source,virtualKey:49,keyDown:true);d?.flags = .maskSecondaryFn;d?.post(tap:.cghidEventTap)
            let u=CGEvent(keyboardEventSource:source,virtualKey:49,keyDown:false);u?.flags = .maskSecondaryFn;u?.post(tap:.cghidEventTap)
            let up=CGEvent(keyboardEventSource:source,virtualKey:63,keyDown:false);up?.type = .flagsChanged;up?.flags = [];up?.post(tap:.cghidEventTap)
            Thread.sleep(forTimeInterval:0.15);return ["delta":keyCount() &- before]
        }
        if mode=="mouse" {
            guard let rect=elementRect(window) else{return ["error":"No fixture bounds"]}
            let old=CGEvent(source:nil)?.location;let point=CGPoint(x:rect.minX+60,y:rect.minY+100);let source=CGEventSource(stateID:.combinedSessionState)
            CGEvent(mouseEventSource:source,mouseType:.leftMouseDown,mouseCursorPosition:point,mouseButton:.left)?.post(tap:.cghidEventTap)
            CGEvent(mouseEventSource:source,mouseType:.leftMouseUp,mouseCursorPosition:point,mouseButton:.left)?.post(tap:.cghidEventTap)
            if let old{CGEvent(mouseEventSource:source,mouseType:.mouseMoved,mouseCursorPosition:old,mouseButton:.left)?.post(tap:.cghidEventTap)}
            Thread.sleep(forTimeInterval:0.08);return ["ok":true]
        }
        if mode=="key" {
            let source=CGEventSource(stateID:.combinedSessionState)
            CGEvent(keyboardEventSource:source,virtualKey:123,keyDown:true)?.post(tap:.cghidEventTap)
            CGEvent(keyboardEventSource:source,virtualKey:123,keyDown:false)?.post(tap:.cghidEventTap)
            Thread.sleep(forTimeInterval:0.08);return ["ok":true]
        }
        return ["error":"Invalid fixture request"]
    case "permissions": return ["accessibility": AXIsProcessTrusted()]
    case "dictionary_import":
        let panel=NSOpenPanel();panel.title="导入 Vao 词典 / Import dictionary";panel.allowedContentTypes=[.plainText,.json];panel.allowsMultipleSelection=false;panel.canChooseDirectories=false
        NSApplication.shared.setActivationPolicy(.accessory);NSApplication.shared.activate(ignoringOtherApps:true)
        guard panel.runModal() == .OK,let url=panel.url else{return ["cancelled":true]}
        return ["path":url.path]
    case "dictionary_export":
        let panel=NSSavePanel();panel.title="导出 Vao 词典 / Export dictionary";panel.allowedContentTypes=[.json,.plainText];panel.nameFieldStringValue="Vao-dictionary.json"
        NSApplication.shared.setActivationPolicy(.accessory);NSApplication.shared.activate(ignoringOtherApps:true)
        guard panel.runModal() == .OK,let url=panel.url else{return ["cancelled":true]}
        return ["path":url.path]
    case "context_start":
        guard let expires=request["expires_ms"] as? Double,Date().timeIntervalSince1970*1000<expires,
              let token=request["token"] as? String,let original=capturedElements[token] else{return ["accepted":false]}
        return ["accepted":startContext(token,original,request["expanded"] as? Bool ?? false)]
    case "target":
        let selection=request["selection"] as? Bool ?? false
        return target(selection,capture:selection)
    case "copy":
        guard let text=request["text"] as? String else {return ["error":"missing text"]}
        NSPasteboard.general.clearContents();NSPasteboard.general.setString(text,forType:.string)
        return ["status":"copied"]
    case "return_source":
        guard let expected=request["target"] as? [String:Any],let pid=expected["pid"] as? Int,
              let token=expected["captureToken"] as? String,
              let original=capturedElements.removeValue(forKey:token) else{return ["error":"Original source expired"]}
        if let error=activateLockedWindow(original,pid_t(pid)){return ["error":error]}
        return ["status":"source_restored"]
    case "deliver":
        guard AXIsProcessTrusted() else {return ["error":"需要辅助功能权限 / Accessibility permission required"]}
        guard let expected=request["target"] as? [String:Any],let text=request["text"] as? String else {return ["error":"missing target"]}
        guard let pid=expected["pid"] as? Int,let token=expected["captureToken"] as? String else{return ["error":"未找到可用的输入框，结果已保留，请复制 / Input field unavailable; copy the result"]}
        guard let original=capturedElements.removeValue(forKey:token) else{return ["error":"原目标已失效，请复制结果 / Original target expired"]}
        // New dictation sessions explicitly request returning to their captured
        // field. Legacy direct-paste callers retain their foreground guards.
        let returning=request["return_to_target"] as? Bool == true && original.snapshot != nil
        if returning,let error=restoreLockedInput(original,pid_t(pid)){return ["error":error]}
        guard frontmost()?.processIdentifier==pid_t(pid) else{return ["error":"当前应用已变化，结果已保留 / Active application changed"]}
        let deliveryMouse=mouseCount(),deliveryKeys=keyCount()
        let process=AXUIElementCreateApplication(pid_t(pid));AXUIElementSetMessagingTimeout(process,0.3)
        let allowedMouse=UInt32(clamping:request["allowed_mouse_events"] as? Int ?? 0)
        if original.composer || original.windowPaste {
            let allowed=UInt32(clamping:request["allowed_key_events"] as? Int ?? 0)
            guard (returning || (mouseCount() &- original.mouseCount==allowedMouse && keyCount() &- original.keyCount==allowed)),
                  let oldWindow=original.window,let newWindow=axElement(attribute(process,kAXFocusedWindowAttribute)),CFEqual(oldWindow,newWindow) else{return ["error":"窗口或会话操作已变化，请复制结果 / Conversation context changed; copy the result"]}
            if original.composer {
            guard AXUIElementSetAttributeValue(original.element,kAXFocusedAttribute as CFString,kCFBooleanTrue) == .success else{return ["error":"无法聚焦消息输入框，请复制结果 / Could not focus the message field"]}
            Thread.sleep(forTimeInterval:0.04)
            }
        }
        if !original.windowPaste {
        guard let currentElement=focusedElement(process,pid_t(pid)),CFEqual(original.element,currentElement) else{return ["error":"输入控件已变化，请复制结果 / Input field changed"]}
        let current=target(true,allowCopy:expected["selection_source"] as? String == "copy",discover:false)
        if original.composer {
            guard (current["selection"] as? String) == nil,(current["length"] as? Int ?? 0)==0 else{return ["error":"输入框中存在选区，请复核后复制 / Review the selected input text"]}
        } else {
            guard sameTarget(expected,current),(expected["selection"] as? String ?? "") == (current["selection"] as? String ?? "") else{return ["error":"目标或光标已变化，结果已保留，请复制 / Target changed; copy the result"]}
        }
        }
        let pb=NSPasteboard.general
        let saved=pb.pasteboardItems?.map { item -> [(NSPasteboard.PasteboardType,Data)] in item.types.compactMap { type in item.data(forType:type).map{(type,$0)} } } ?? []
        guard let pasteCode=pasteKeyCode() else{return ["error":"无法确认当前键盘布局的粘贴键，请复制结果 / Paste key unavailable for this keyboard layout"]}
        let item=NSPasteboardItem();item.setString(text,forType:.string);item.setData(Data(),forType:NSPasteboard.PasteboardType("org.nspasteboard.TransientType"));item.setData(Data(),forType:NSPasteboard.PasteboardType("org.nspasteboard.ConcealedType"))
        pb.clearContents();pb.writeObjects([item]);let count=pb.changeCount
        guard frontmost()?.processIdentifier==pid_t(pid), (original.windowPaste ? (axElement(attribute(process,kAXFocusedWindowAttribute)).map{CFEqual(original.element,$0)} ?? false) : (focusedElement(process,pid_t(pid)).map{CFEqual(original.element,$0)} ?? false)), (original.composer || original.windowPaste || sameTarget(expected,target(false,discover:false))) else {if pb.changeCount==count {restore(saved)};return ["error":"焦点发生变化，未发送粘贴 / Focus changed"]}
        if returning {
            guard mouseCount()==deliveryMouse,keyCount()==deliveryKeys,
                  original.snapshot?.matches(original.element)==true else {
                if pb.changeCount==count {restore(saved)}
                return ["error":"原输入位置再次变化，未发送粘贴 / Original input changed during delivery"]
            }
        } else if original.composer || original.windowPaste {
            let allowed=UInt32(clamping:request["allowed_key_events"] as? Int ?? 0)
            guard mouseCount() &- original.mouseCount==allowedMouse,keyCount() &- original.keyCount==allowed else{if pb.changeCount==count{restore(saved)};return ["error":"会话操作已变化，未发送粘贴 / Conversation interaction changed"]}
        }
        let source=CGEventSource(stateID:.combinedSessionState)
        guard let down=CGEvent(keyboardEventSource:source,virtualKey:pasteCode,keyDown:true),let up=CGEvent(keyboardEventSource:source,virtualKey:pasteCode,keyDown:false) else {restore(saved);return ["error":"无法生成输入事件"]}
        down.flags = .maskCommand;up.flags = .maskCommand;down.post(tap:.cghidEventTap);up.post(tap:.cghidEventTap)
        Thread.sleep(forTimeInterval:0.35)
        if pb.changeCount==count {restore(saved)}
        return ["status":"paste_sent"]
    default:return ["error":"unknown action"]
    }
}
func restore(_ saved:[[(NSPasteboard.PasteboardType,Data)]]) {
    let pb=NSPasteboard.general;pb.clearContents()
    let items=saved.map { entries -> NSPasteboardItem in let item=NSPasteboardItem();for (type,data) in entries{item.setData(data,forType:type)};return item }
    if !items.isEmpty {pb.writeObjects(items)}
}
func respond(_ data:Data) {
    let result:[String:Any]
    if let request=(try? JSONSerialization.jsonObject(with:data)) as? [String:Any] {result=run(request)} else {result=["error":"invalid request"]}
    var output=(try? JSONSerialization.data(withJSONObject:result,options:[.sortedKeys])) ?? Data("{}".utf8)
    output.append(10);FileHandle.standardOutput.write(output)
}
if CommandLine.arguments.contains("--verify-context-fixtures") {
    let value="邮件🙂e\u{301}，"+String(repeating:"技术术语",count:400)
    let cursor=value.index(value.startIndex,offsetBy:5)
    let result=croppedContext(value,cursor)
    let before=result["before_cursor"] as! String;let after=result["after_cursor"] as! String
    let valid=before.count<=500 && after.count<=500 && before.utf8.count+after.utf8.count<=8192
      && contextCursor("a🙂b",2)==nil && contextCursor("a🙂b",3) != nil
      && !(before+after).contains("�")
    let output:[String:Any]=["unicode_and_bounds":valid,"raw_source_recorded":false,"paste_layout_found":pasteKeyCode() != nil,"sensitive_apps_skipped":sensitiveApplication("com.apple.Passwords") && sensitiveApplication("com.agilebits.onepassword7") && !sensitiveApplication("com.example.editor")]
    FileHandle.standardOutput.write(try! JSONSerialization.data(withJSONObject:output));exit(valid ? 0 : 1)
}
if CommandLine.arguments.contains("--daemon") {
    while let line=readLine(){autoreleasepool{respond(Data(line.utf8))}}
}else{respond(FileHandle.standardInput.readDataToEndOfFile())}
