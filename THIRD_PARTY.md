# Third-party notices

## Protocol tables

The MDR v2 opcode values, inquired types, enum values and payload layouts in `crates/mdr-proto` were ported from the generated headers of **mos9527/SonyHeadphonesClient** (`libmdr`, branch `v1-compat`), which are licensed under the MIT License:

```
MIT License

Copyright (c) 2026 mos9527, Amr Satrio and other contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Byte-level behaviour on the WH-ULT900N was cross-checked against the public notes of owaiskaifi/sony-connect, Leonard013/sony-ult-ctl, cristianoashok/Z-ULT-Connect, pats01/sony-ult-wear-omarchy and AmitRajput-Dev/SonyBridge. Gadgetbridge (AGPL-3.0) and Resonance for Sound Connect (PolyForm Noncommercial) were read as references only; no code from either is included.

## QR codes

The desktop app draws the QR beside an equalizer share code with **qrcode-generator** 2.0.4 by Kazuhiko Arase (https://github.com/kazuhikoarase/qrcode-generator), bundled into the frontend and licensed under the MIT License:

```
Copyright (c) 2009 Kazuhiko Arase

Licensed under the MIT license:
  http://www.opensource.org/licenses/mit-license.php

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

The word "QR Code" is a registered trademark of DENSO WAVE INCORPORATED.

## Fonts

- Instrument Sans, Doto and DM Mono are used under the SIL Open Font License 1.1. Their license files are bundled next to the font files in `apps/desktop/src/assets/fonts/`.

## Trademarks

Sony, ULT WEAR, ULT POWER SOUND, DSEE and LDAC are trademarks of Sony Group Corporation or their respective owners. This project is not affiliated with or endorsed by Sony.
