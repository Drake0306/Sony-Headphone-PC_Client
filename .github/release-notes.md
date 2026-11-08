SHDC (Sony Headphone Desktop Client) controls Sony ULT WEAR (WH-ULT900N) headphones from the menu bar or tray: noise control, ambient level, ULT, equalizer, battery, scenes with desktop triggers, hotkeys, an advanced ten-band equalizer with shareable codes, and a command-line tool.

## What changed
- Windows: the main window opened white. It was created from inside the popover's message callback, where a second WebView2 is set up with a nested message loop and may never paint. The window is now created as its own task on the main thread, kept hidden until its page has loaded, and shown after 1.5 seconds regardless, so a page that fails still shows a window.
- When the page itself breaks, the window now shows the error instead of staying blank, and the same text goes to `shdc.log` in the app's log folder (macOS `~/Library/Logs/com.drake0306.hairline/`, Windows `%LOCALAPPDATA%\com.drake0306.hairline\logs\`), together with a line for each main window creation and page load.
- With the Advanced panel on, the tray menu gains "Web inspector", which opens the main window's developer tools.

## Updating from an earlier version
Settings → Updates → Update now. On a Mac the file in Applications keeps the name it was installed under (for example `Hairline.app`), because the update replaces the contents, not the file; rename it in Finder if you like.

## Install on macOS
1. Open the `.dmg` and drag SHDC to Applications.
2. Open SHDC once. macOS says it could not verify the app, because it is not signed with an Apple Developer certificate yet. Click Done.
3. Open System Settings → Privacy & Security, scroll to Security, and click Open Anyway next to the SHDC message. Enter your password, then click Open. After that it opens normally.
   Terminal alternative: `xattr -dr com.apple.quarantine /Applications/SHDC.app`
4. Pair the headphones in System Settings → Bluetooth first. SHDC connects on its own.

## Install on Windows
1. Run the `.msi` or the `-setup.exe`.
2. If SmartScreen appears, choose More info → Run anyway. The installer is not signed yet.
3. The Windows build compiles and passes its tests, but it has not been tried with real headphones on Windows.

## Known limits
- Tested on a real WH-ULT900N on macOS. Other Sony models connect but noise control and the equalizer show as unavailable on them.
- The advanced equalizer runs its ten bands in SHDC; "Send best fit" writes the closest six-band match to the headphones.
