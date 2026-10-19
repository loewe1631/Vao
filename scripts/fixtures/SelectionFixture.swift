import AppKit
let app=NSApplication.shared
app.setActivationPolicy(.regular)
let menu=NSMenu();let application=NSMenuItem();application.submenu=NSMenu();menu.addItem(application)
let edit=NSMenuItem(title:"Edit",action:nil,keyEquivalent:"");let items=NSMenu(title:"Edit")
items.addItem(withTitle:"Copy",action:#selector(NSText.copy(_:)),keyEquivalent:"c");edit.submenu=items;menu.addItem(edit);app.mainMenu=menu
let w=NSWindow(contentRect:NSRect(x:100,y:100,width:480,height:220),styleMask:[.titled,.closable],backing:.buffered,defer:false)
w.title=ProcessInfo.processInfo.environment["VAO_FIXTURE_LABEL"] ?? "Vao Synthetic Selection"
let view=NSTextView(frame:NSRect(x:0,y:0,width:480,height:220));view.string="合成选区：明天发送报告。";w.contentView=view;w.makeKeyAndOrderFront(nil);w.makeFirstResponder(view)
view.setSelectedRange(NSRange(location:0,length:CommandLine.arguments.contains("--cursor") ? 0 : view.string.utf16.count));app.activate(ignoringOtherApps:true)
DispatchQueue.main.asyncAfter(deadline:.now()+1){view.setSelectedRange(NSRange(location:0,length:CommandLine.arguments.contains("--cursor") ? 0 : view.string.utf16.count))}
app.run()
