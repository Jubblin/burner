# Third-party notices

Burner itself is Apache-2.0 (see LICENSE). The components below keep their own licences.

## Fonts (`ui/fonts`)

Satoshi was used earlier but removed: its licence does not allow redistributing the font files in a public repository. Earlier commits still contain them.

### JetBrains Mono (`JetBrainsMono-*.woff2`)
SIL Open Font License 1.1, copyright and full text below. It permits redistribution (including inside an app) provided this notice and licence travel with the font files.

```
Copyright 2020 The JetBrains Mono Project Authors (https://github.com/JetBrains/JetBrainsMono)

This Font Software is licensed under the SIL Open Font License, Version 1.1.
This license is copied below, and is also available with a FAQ at:
https://openfontlicense.org


-----------------------------------------------------------
SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007
-----------------------------------------------------------

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The
fonts, including any derivative works, can be bundled, embedded, 
redistributed and/or sold with any software provided that any reserved
names are not used by derivative works. The fonts and derivatives,
however, cannot be released under any other type of license. The
requirement for fonts to remain under this license does not apply
to any document created using the fonts or their derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may
include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical
writer or other person who contributed to the Font Software.

PERMISSION & CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining
a copy of the Font Software, to use, study, copy, merge, embed, modify,
redistribute, and sell modified and unmodified copies of the Font
Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components,
in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy
contains the above copyright notice and this license. These can be
included either as stand-alone text files, human-readable headers or
in the appropriate machine-readable metadata fields within text or
binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font
Name(s) unless explicit written permission is granted by the corresponding
Copyright Holder. This restriction only applies to the primary font name as
presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font
Software shall not be used to promote, endorse or advertise any
Modified Version, except to acknowledge the contribution(s) of the
Copyright Holder(s) and the Author(s) or with their explicit written
permission.

5) The Font Software, modified or unmodified, in part or in whole,
must be distributed entirely under this license, and must not be
distributed under any other license. The requirement for fonts to
remain under this license does not apply to any document created
using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are
not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE
COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.
```

### Satoshi (`Satoshi-*.woff2`): REDISTRIBUTION NOT PERMITTED IN A PUBLIC REPO
Licensed under the Indian Type Foundry Free Font License (FFL) v2.0, https://www.fontshare.com (licence file ships in the official download at `https://api.fontshare.com/v2/fonts/download/satoshi`, `License/FFL.txt`). Copyright (c) Indian Type Foundry. The full text is not reproduced here because the licence forbids redistribution; read it at the source.

Quoted terms (section 02, Limitations of Usage):
- "The Font Software may not, beyond the permitted copies and uses defined herein, be distributed, duplicated, ... or otherwise made available to any other person or entity, whether for free or for a fee. This includes distributing the Font Software through another font website, font library, marketplace, **repository**, download service, application or platform, ... **publicly accessible servers**, ..."
- "You may not modify, edit, adapt, ... This includes modifying or replacing glyphs, **subsetting, format conversion**, or altering font names, copyright information, ownership information or other metadata."
- Allowed (section 01): "You may self-host the Font Software on your own servers or infrastructure for use on your own websites and applications" and "You may embed the Font Software in mobile or desktop applications ... for the uses permitted under this License."

Verdict:
- Raw font files in this public repository: **NO**. A public GitHub repo is a "repository"/"publicly accessible server".
- Inside the app bundle: **probably yes** for the app's own use (embedding in a desktop app is granted), but the shipped files must be the unmodified official files; if the bundled `.woff2` files are converted or subset from the official download, that is itself prohibited ("subsetting, format conversion").

Options (not applied; nothing was deleted): (a) fetch Satoshi from the official source at build time and git-ignore `ui/fonts/Satoshi-*`; (b) switch to an OFL face from DESIGN.md's verified list.

## Rust dependencies
Summary from `cargo deny list` (all licences pass `cargo deny check licenses`): MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2/3-Clause, ISC, MPL-2.0, Unicode-3.0, Zlib, Unlicense, 0BSD, BSL-1.0, CC0-1.0, MIT-0, CDLA-Permissive-2.0. `r-efi` is offered as MIT OR Apache-2.0 OR LGPL-2.1-or-later (used under MIT/Apache). MPL-2.0 crates (cssparser, selectors, dtoa-short, option-ext) are unmodified. Regenerate with `cd src-tauri && cargo deny list`.
