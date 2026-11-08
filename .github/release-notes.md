Hairline controls Sony ULT WEAR (WH-ULT900N) headphones from the menu bar or tray: noise control, ambient level, ULT, equalizer, battery, scenes with desktop triggers, hotkeys, an advanced ten-band equalizer with shareable codes, and a command-line tool.

## Install on macOS
1. Open the `.dmg` and drag Hairline to Applications.
2. First launch: right-click Hairline and choose Open, then Open again. macOS asks because the app is not signed with an Apple Developer certificate yet. After that it opens normally.
3. Pair the headphones in System Settings → Bluetooth first. Hairline connects on its own.

## Install on Windows
1. Run the `.msi` or the `-setup.exe`.
2. If SmartScreen appears, choose More info → Run anyway. The installer is not signed yet.
3. The Windows build compiles and passes its tests, but it has not been tried with real headphones on Windows.

## Known limits
- Tested on a real WH-ULT900N on macOS. Other Sony models connect but noise control and the equalizer show as unavailable on them.
- The advanced equalizer runs its ten bands in Hairline; "Send best fit" writes the closest six-band match to the headphones.
