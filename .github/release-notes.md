SHDC (Sony Headphone Desktop Client) controls Sony ULT WEAR (WH-ULT900N) headphones from the menu bar or tray: noise control, ambient level, ULT, equalizer, battery, scenes with desktop triggers, hotkeys, an advanced ten-band equalizer with shareable codes, and a command-line tool.

## What changed
- Updates keep working when github.com's download host fails. On 8 November the check said "Could not reach GitHub" although 0.3.0 was out, because GitHub's release download host returned 504 from one region while the rest of GitHub answered. The app now reads the feed from a second GitHub host as well, and downloads through GitHub's API, which routes around that host.
- The failure message now says to try again in a few minutes. "Up to date" is still only shown when GitHub answered.

## Updating from Hairline 0.2.x or SHDC 0.3.0
Settings → Updates → Update now. Those versions only know the first feed, so during a GitHub outage like the one above the check fails until GitHub recovers; after that the update installs normally. From 0.3.1 on, the fallback is built in.
On a Mac the file in Applications keeps the name it was installed under (for example `Hairline.app`), because the update replaces the contents, not the file. Rename it in Finder if you like, or drag `SHDC.app` from the `.dmg` over it.

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
