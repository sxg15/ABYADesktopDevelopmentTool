# ABYA Windows terminal patch

Upstream: portable-pty 0.9.0, WezTerm, MIT license (LICENSE.md retained).
Source copied from the already installed crates.io package, without modifying the global Cargo cache.

Only behavioral patch: src/win/psuedocon.rs omits PSEUDOCONSOLE_INHERIT_CURSOR.
The embedded xterm view is an independent terminal surface. It must not inherit the cursor of a
previous console or depend on a GUI round trip while Windows creates/closes a pseudoconsole.
Resize-quirk and Win32 input flags, process/environment setup and all non-Windows code are unchanged.

Microsoft documents the asynchronous cursor-query requirement and possible hangs:
https://learn.microsoft.com/en-us/windows/console/createpseudoconsole

The application test embedded_pty_starts_without_a_frontend_cursor_reply exercises a real Windows
PTY with no frontend attached. When updating portable-pty, review this patch and rerun startup,
resize, pause/reconnect and unsent-input APP tests. Do not silently drop the patch on upgrade.
