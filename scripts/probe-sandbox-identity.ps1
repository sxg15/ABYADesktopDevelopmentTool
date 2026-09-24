$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Security.Principal;
public static class AbyaTokenProbe {
 [DllImport("advapi32", SetLastError=true)] static extern bool GetTokenInformation(IntPtr h, int c, IntPtr b, int n, out int size);
 public static string[] Restricted() {
  using(var identity=WindowsIdentity.GetCurrent()) {
   int size; GetTokenInformation(identity.Token, 11, IntPtr.Zero, 0, out size);
   var p=Marshal.AllocHGlobal(size);
   try {
    if(!GetTokenInformation(identity.Token, 11, p, size, out size)) throw new Exception("Token probe failed");
    int count=Marshal.ReadInt32(p); var result=new string[count];
    for(int i=0;i<count;i++) result[i]=new SecurityIdentifier(Marshal.ReadIntPtr(p, IntPtr.Size+i*(IntPtr.Size==8?16:8))).Value;
    return result;
   } finally { Marshal.FreeHGlobal(p); }
  }
 }
}
'@
[System.Security.Principal.WindowsIdentity]::GetCurrent().Name
[AbyaTokenProbe]::Restricted()
