<!--
  OpenBrush
  Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
  Author: Aryan Sinha <techyminati>
  Licensed under the CipherOS License 2.0. See LICENSE for the full terms.
-->

# Third-party notices

OpenBrush's own source code is covered by the CipherOS License 2.0 (see `LICENSE`).
The components below are developed by third parties and remain under their own
licences, which are reproduced or referenced here as those licences require.

## Rust crates

OpenBrush links against the following crates (and their transitive dependencies),
all available under permissive licences (MIT and/or Apache-2.0, or similar):

| Crate | Licence |
|-------|---------|
| eframe, egui, egui_extras | MIT OR Apache-2.0 |
| tiny-skia | BSD-3-Clause |
| image | MIT OR Apache-2.0 |
| rfd | MIT |
| arboard | MIT OR Apache-2.0 |
| cosmic-text | MIT OR Apache-2.0 |

Run `cargo tree` for the full dependency list.

## Icons — Fluent UI System Icons

The SVG files in `assets/icons/` are taken from
[Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons)
(npm package `@fluentui/svg-icons` 1.1.344), Copyright (c) 2020 Microsoft Corporation,
licensed under the MIT License (full text in `assets/icons/LICENSE`).
They were renamed and given a white fill so the app can tint them for each theme.

## Font — Selawik

`assets/fonts/selawk.ttf` and `assets/fonts/selawksb.ttf` are
[Selawik](https://github.com/microsoft/Selawik), Copyright 2015 Microsoft Corporation,
licensed under the SIL Open Font License 1.1 (full text in `assets/fonts/OFL.txt`).
"Selawik" is a Reserved Font Name and a trademark of Microsoft Corporation.
