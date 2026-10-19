import AppKit
let app=NSApplication.shared;app.setActivationPolicy(.regular)
let menu=NSMenu();let application=NSMenuItem();application.submenu=NSMenu();menu.addItem(application)
let edit=NSMenuItem(title:"Edit",action:nil,keyEquivalent:"");let items=NSMenu(title:"Edit")
items.addItem(withTitle:"Copy",action:#selector(NSText.copy(_:)),keyEquivalent:"c")
items.addItem(withTitle:"Paste",action:#selector(NSText.paste(_:)),keyEquivalent:"v");edit.submenu=items;menu.addItem(edit);app.mainMenu=menu
let w=NSWindow(contentRect:NSRect(x:100,y:100,width:640,height:420),styleMask:[.titled,.closable],backing:.buffered,defer:false)
w.title="Vao Synthetic Composer"
let content=NSView(frame:NSRect(x:0,y:0,width:640,height:420));w.contentView=content
let other=NSButton(frame:NSRect(x:20,y:330,width:130,height:40));other.title="Synthetic sidebar";content.addSubview(other)
let ambiguous=Bundle.main.bundleIdentifier=="app.vaovao.fixtureambiguous"
let input=NSTextView(frame:NSRect(x:180,y:20,width:ambiguous ? 200 : 430,height:120));content.addSubview(input)
if ambiguous{let second=NSTextView(frame:NSRect(x:400,y:20,width:220,height:120));content.addSubview(second)}
w.makeKeyAndOrderFront(nil);w.makeFirstResponder(Bundle.main.bundleIdentifier=="app.vaovao.fixtureopaque" ? input : other);app.activate(ignoringOtherApps:true)
app.run()
