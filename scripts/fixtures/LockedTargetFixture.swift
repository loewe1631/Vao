import AppKit
// Isolated fixture. Control files exist only beside the native regression output.
let app=NSApplication.shared
app.setActivationPolicy(.regular)
let browser=Bundle.main.bundleIdentifier=="app.vaovao.fixturebrowser"
let control=URL(fileURLWithPath:CommandLine.arguments[CommandLine.arguments.firstIndex(of:"--control")!+1])
let response=control.appendingPathExtension("response")
let menu=NSMenu();let root=NSMenuItem();root.submenu=NSMenu();menu.addItem(root)
let edit=NSMenuItem(title:"Edit",action:nil,keyEquivalent:"");let items=NSMenu(title:"Edit")
items.addItem(withTitle:"Copy",action:#selector(NSText.copy(_:)),keyEquivalent:"c")
items.addItem(withTitle:"Paste",action:#selector(NSText.paste(_:)),keyEquivalent:"v")
edit.submenu=items;menu.addItem(edit);app.mainMenu=menu
var windows:[String:NSWindow]=[:], fields:[String:NSTextView]=[:]
for (i,label) in (browser ? ["R"] : ["A","B"]).enumerated() {
 let w=NSWindow(contentRect:NSRect(x:120+i*100,y:180+i*90,width:540,height:300),styleMask:[.titled,.closable,.miniaturizable],backing:.buffered,defer:false)
 w.isReleasedWhenClosed=false;w.title="Vao Synthetic \(label)";w.identifier=NSUserInterfaceItemIdentifier(label)
 for n in 1...(browser ? 1 : 2) {
  let v=NSTextView(frame:NSRect(x:20,y:n==1 ? 155 : 20,width:500,height:115))
  v.setAccessibilityIdentifier("\(label)\(n)");v.isRichText=false;v.string="\(label)\(n) seed";w.contentView!.addSubview(v);fields["\(label)\(n)"]=v
 }
 windows[label]=w;w.orderFront(nil)
}
func state(_ sequence:Int,_ error:String="")->[String:Any] {
 var values:[String:Any]=[:]
 for (name,v) in fields {values[name]=["text":v.string,"offset":v.selectedRange().location,"length":v.selectedRange().length]}
 return ["sequence":sequence,"pid":ProcessInfo.processInfo.processIdentifier,"fields":values,"key_window":app.keyWindow?.identifier?.rawValue ?? "","error":error]
}
var lastSequence = -1
let timer=Timer.scheduledTimer(withTimeInterval:0.02,repeats:true){_ in
 guard let data=try? Data(contentsOf:control),let req=(try? JSONSerialization.jsonObject(with:data)) as? [String:Any],let seq=req["sequence"] as? Int,seq != lastSequence else{return}
 lastSequence=seq
 let field=req["field"] as? String ?? (browser ? "R1" : "A1")
 let v=fields[field];let w=windows[String(field.prefix(1))]
 switch req["action"] as? String ?? "" {
 case "reset":
  for (name,value) in fields {value.string=name=="A1" ? "prefix suffix" : "\(name) seed";value.setSelectedRange(NSRange(location:name=="A1" ? 7 : value.string.utf16.count,length:0))}
  w?.deminiaturize(nil);w?.makeKeyAndOrderFront(nil);w?.makeFirstResponder(v);app.activate(ignoringOtherApps:true)
 case "focus": w?.deminiaturize(nil);w?.makeKeyAndOrderFront(nil);w?.makeFirstResponder(v);app.activate(ignoringOtherApps:true)
 case "change": if let v {v.string="changed original";v.setSelectedRange(NSRange(location:2,length:0))}
 case "caret": v?.setSelectedRange(NSRange(location:1,length:0))
 case "select": if let v {v.setSelectedRange(NSRange(location:0,length:6))}
 case "close": w?.close()
 case "minimize": w?.miniaturize(nil)
 default: break
 }
 if let output=try? JSONSerialization.data(withJSONObject:state(seq),options:[.sortedKeys]) {try? output.write(to:response,options:.atomic)}
}
app.run()
