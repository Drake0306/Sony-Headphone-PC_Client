SHDC (Sony Headphone Desktop Client) controls Sony ULT WEAR (WH-ULT900N) headphones from the menu bar or tray: noise control, ambient level, ULT, equalizer, battery, scenes with desktop triggers, hotkeys, an advanced ten-band equalizer with shareable codes, and a command-line tool.

## What changed
- Ten new appearance themes next to Paper and Ink, picked from swatches in Settings → Appearance. Light: Latte, Solarized, Dawn, Sepia, Kraft. Dark: Nord, Mocha, Gruvbox, Tokyo Night, OLED Black. Each theme brings its own accent for live controls; Kraft and OLED Black keep ULT orange. System still follows the OS between Paper and Ink.
- Settings files survive theme ids this build does not know: an unknown id reads as Paper and every other setting stays. Going back to 0.3.2 or older after choosing a new theme is the one exception: those builds reset the settings file when they meet the new id.

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
