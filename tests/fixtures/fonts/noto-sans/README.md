# Noto Sans, subset (test fixtures)

These fonts are subsets of Noto Sans, by the Noto Project Authors, and
are licensed under the SIL Open Font License, Version 1.1 (OFL.txt), not
under isolex's Apache License: every file in this directory is, including
the isolex font files converted from them. Noto is a trademark of Google
LLC. Noto Sans declares no Reserved Font Name.

They are test data for `isolex font import` and `isolex font export`,
and no part of isolex.

Sources:

- `NotoSans-Regular.ttf`, `NotoSans-Regular.otf` and `NotoSans-Bold.ttf`
  from the unhinted fonts of the NotoSans-v2.015 release of
  <https://github.com/notofonts/latin-greek-cyrillic>;
- `NotoSans[wdth,wght].ttf` from <https://github.com/google/fonts>
  (`ofl/notosans`).

Subset with fontTools 4.67.0, keeping every name record (the notices
among them) and the `kern` and `liga` features:

```sh
o="--layout-features=kern,liga --name-IDs=*"
pyftsubset NotoSans-Regular.ttf --text="<U+0020–U+007E>éÅ" $o --output-file=noto-sans-regular.ttf
pyftsubset NotoSans-Regular.otf --text="AVTaefilo 0123456789éÅ" $o --output-file=noto-sans-regular.otf
pyftsubset NotoSans-Bold.ttf --text="AVTaefilo 0123456789éÅ" $o --output-file=noto-sans-bold.ttf
pyftsubset NotoSans-Regular.ttf --text="AVTaefilo 0123456789éÅ" $o --flavor=woff --output-file=noto-sans-regular.woff
pyftsubset "NotoSans[wdth,wght].ttf" --text="AVTaefilo 0123456789éÅ" $o --output-file=noto-sans-vf.ttf
```
