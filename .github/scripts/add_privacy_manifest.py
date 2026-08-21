#!/usr/bin/env python3
"""Register PrivacyInfo.xcprivacy in the generated iOS app target.

`tauri ios init` regenerates the Xcode project on every build, so the privacy
manifest has to be added back each time. Apple rejects at upload when an app
uses a required-reason API without one.

The new entries are anchored on Assets.xcassets rather than on absolute
positions in the file: the asset catalogue is already a resource of the app
target (CI copies the app icons into it), so cloning its wiring puts the
manifest in the same group and the same Copy Bundle Resources phase without
this script needing to know the project's layout. If the anchor is missing the
script fails loudly — shipping an IPA without the manifest just moves the
failure to App Store Connect.
"""

import hashlib
import re
import sys

MANIFEST = "PrivacyInfo.xcprivacy"
ANCHOR = "Assets.xcassets"


def object_id(seed: str) -> str:
    """A 24-hex-digit identifier in Xcode's style, stable for a given seed."""
    return hashlib.sha256(seed.encode()).hexdigest()[:24].upper()


def patch(source: str) -> tuple[str, str]:
    """Return the patched project text and a short description of what changed."""
    if MANIFEST in source:
        return source, "already registered"

    file_ref = re.search(
        r"^\t*([0-9A-F]{24}) /\* %s \*/ = \{isa = PBXFileReference;.*$" % ANCHOR,
        source,
        re.M,
    )
    if not file_ref:
        raise SystemExit(f"error: no PBXFileReference for {ANCHOR}; cannot place {MANIFEST}")

    build_file = re.search(
        r"^\t*([0-9A-F]{24}) /\* %s in Resources \*/ = \{isa = PBXBuildFile;.*$" % ANCHOR,
        source,
        re.M,
    )
    if not build_file:
        raise SystemExit(f"error: no PBXBuildFile for {ANCHOR}; cannot place {MANIFEST}")

    ref_id = object_id(f"fileref:{MANIFEST}")
    build_id = object_id(f"buildfile:{MANIFEST}")

    # 1. Declare the file, beside the anchor's own declaration.
    source = source.replace(
        file_ref.group(0),
        file_ref.group(0)
        + f"\n\t\t{ref_id} /* {MANIFEST} */ = {{isa = PBXFileReference; "
        f'lastKnownFileType = text.plist.xml; path = {MANIFEST}; sourceTree = "<group>"; }};',
        1,
    )

    # 2. Declare it as something the target builds.
    source = source.replace(
        build_file.group(0),
        build_file.group(0)
        + f"\n\t\t{build_id} /* {MANIFEST} in Resources */ = {{isa = PBXBuildFile; "
        f"fileRef = {ref_id} /* {MANIFEST} */; }};",
        1,
    )

    # 3/4. Join the anchor wherever it is listed. A trailing comma distinguishes a
    # membership entry from the declarations above, which continue with " = {".
    groups = source.count(f"{file_ref.group(1)} /* {ANCHOR} */,")
    source = source.replace(
        f"{file_ref.group(1)} /* {ANCHOR} */,",
        f"{file_ref.group(1)} /* {ANCHOR} */,\n\t\t\t\t{ref_id} /* {MANIFEST} */,",
    )
    phases = source.count(f"{build_file.group(1)} /* {ANCHOR} in Resources */,")
    source = source.replace(
        f"{build_file.group(1)} /* {ANCHOR} in Resources */,",
        f"{build_file.group(1)} /* {ANCHOR} in Resources */,"
        f"\n\t\t\t\t{build_id} /* {MANIFEST} in Resources */,",
    )

    if groups == 0 or phases == 0:
        raise SystemExit(
            f"error: {ANCHOR} is declared but not a member of any group ({groups}) "
            f"or resources phase ({phases}); {MANIFEST} would not be copied"
        )

    return source, f"added to {groups} group(s) and {phases} resources phase(s)"


def main() -> None:
    path = sys.argv[1]
    with open(path, encoding="utf-8") as handle:
        source = handle.read()
    patched, note = patch(source)
    if patched != source:
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(patched)
    print(f"{MANIFEST}: {note}")


if __name__ == "__main__":
    main()
