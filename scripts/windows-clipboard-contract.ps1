$ErrorActionPreference = 'Stop'
# Synthetic Win32 clipboard contract; no audio, input events or external apps.
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class VaoClipboardContract {
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern IntPtr CreateWindowExW(uint ex,string cls,string title,uint style,int x,int y,int w,int h,IntPtr parent,IntPtr menu,IntPtr instance,IntPtr data);
 [DllImport("user32.dll")] static extern bool DestroyWindow(IntPtr hwnd);
 [DllImport("user32.dll")] static extern bool OpenClipboard(IntPtr hwnd);
 [DllImport("user32.dll")] static extern bool CloseClipboard();
 [DllImport("user32.dll")] static extern bool EmptyClipboard();
 [DllImport("user32.dll")] static extern uint GetClipboardSequenceNumber();
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern uint RegisterClipboardFormatW(string name);
 [DllImport("user32.dll")] static extern IntPtr SetClipboardData(uint format,IntPtr value);
 [DllImport("user32.dll")] static extern IntPtr GetClipboardData(uint format);
 [DllImport("kernel32.dll")] static extern IntPtr GlobalAlloc(uint flags,UIntPtr size);
 [DllImport("kernel32.dll")] static extern IntPtr GlobalLock(IntPtr memory);
 [DllImport("kernel32.dll")] static extern bool GlobalUnlock(IntPtr memory);
 [DllImport("kernel32.dll")] static extern UIntPtr GlobalSize(IntPtr memory);
 static IntPtr Owner() {var w=CreateWindowExW(0,"STATIC","Synthetic clipboard contract",0,0,0,0,0,new IntPtr(-3),IntPtr.Zero,IntPtr.Zero,IntPtr.Zero);if(w==IntPtr.Zero)throw new Exception("owner unavailable");return w;}
 static void Put(uint format,byte[] bytes) {var h=GlobalAlloc(2,(UIntPtr)bytes.Length);var p=GlobalLock(h);if(p==IntPtr.Zero)throw new Exception("allocation unavailable");Marshal.Copy(bytes,0,p,bytes.Length);GlobalUnlock(h);if(SetClipboardData(format,h)==IntPtr.Zero)throw new Exception("write unavailable");}
 public static object Run() {
  var owner=Owner();if(!OpenClipboard(owner))throw new Exception("clipboard busy");
  EmptyClipboard();Put(13,Encoding.Unicode.GetBytes("Synthetic pending paste\0"));
  var marker=RegisterClipboardFormatW("Vao.Synthetic.Clipboard.Contract");Put(marker,new byte[16]);
  var beforeClose=GetClipboardSequenceNumber();CloseClipboard();var afterClose=GetClipboardSequenceNumber();
  DestroyWindow(owner);var afterDestroy=GetClipboardSequenceNumber();
  owner=Owner();if(!OpenClipboard(owner))throw new Exception("read busy");
  var afterOpen=GetClipboardSequenceNumber();var data=GetClipboardData(marker);var markerSize=data==IntPtr.Zero?0:GlobalSize(data).ToUInt64();
  var afterRead=GetClipboardSequenceNumber();CloseClipboard();var afterReadClose=GetClipboardSequenceNumber();DestroyWindow(owner);
  return new {before_close=beforeClose,after_close=afterClose,after_owner_destroy=afterDestroy,after_open=afterOpen,after_marker_read=afterRead,after_read_close=afterReadClose,marker_present=data!=IntPtr.Zero,marker_size=markerSize,audio_captured=false,input_dispatched=false};
 }
}
'@
$result = [VaoClipboardContract]::Run()
$result | ConvertTo-Json -Compress
if (!$result.marker_present -or $result.marker_size -lt 16 -or
    $result.after_close -ne $result.after_owner_destroy -or $result.after_close -ne $result.after_open -or
    $result.after_close -ne $result.after_marker_read -or $result.after_close -ne $result.after_read_close) {
    throw 'Synthetic clipboard ownership could not be sealed after write-close'
}
