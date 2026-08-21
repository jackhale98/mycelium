#!/usr/bin/env python3
"""Tests for add_privacy_manifest.py, run by CI."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from add_privacy_manifest import MANIFEST, patch  # noqa: E402

# Shaped like the project `tauri ios init` generates: the asset catalogue is
# declared once and referenced from a group and the resources phase.
PROJECT = """// !$*UTF8*$!
{
	objects = {

/* Begin PBXBuildFile section */
		1A2B3C4D5E6F708192A3B4C5 /* Assets.xcassets in Resources */ = {isa = PBXBuildFile; fileRef = 0F1E2D3C4B5A697887665544 /* Assets.xcassets */; };
		AAAA1111BBBB2222CCCC3333 /* main.rs in Sources */ = {isa = PBXBuildFile; fileRef = DDDD4444EEEE5555FFFF6666 /* main.rs */; };
/* End PBXBuildFile section */

/* Begin PBXFileReference section */
		0F1E2D3C4B5A697887665544 /* Assets.xcassets */ = {isa = PBXFileReference; lastKnownFileType = folder.assetcatalog; path = Assets.xcassets; sourceTree = "<group>"; };
		DDDD4444EEEE5555FFFF6666 /* main.rs */ = {isa = PBXFileReference; lastKnownFileType = text; path = main.rs; sourceTree = "<group>"; };
/* End PBXFileReference section */

/* Begin PBXGroup section */
		9999888877776666555544441 = {
			isa = PBXGroup;
			children = (
				0F1E2D3C4B5A697887665544 /* Assets.xcassets */,
				DDDD4444EEEE5555FFFF6666 /* main.rs */,
			);
			sourceTree = "<group>";
		};
/* End PBXGroup section */

/* Begin PBXResourcesBuildPhase section */
		7777666655554444333322221 = {
			isa = PBXResourcesBuildPhase;
			buildActionMask = 2147483647;
			files = (
				1A2B3C4D5E6F708192A3B4C5 /* Assets.xcassets in Resources */,
			);
			runOnlyForDeploymentPostprocessing = 0;
		};
/* End PBXResourcesBuildPhase section */
	};
}
"""


class TestPatch(unittest.TestCase):
    def test_registers_the_manifest(self):
        out, note = patch(PROJECT)
        self.assertIn("1 group(s)", note)
        self.assertIn("1 resources phase(s)", note)
        self.assertIn("PBXFileReference; lastKnownFileType = text.plist.xml", out)
        self.assertIn(f"{MANIFEST} in Resources */ = {{isa = PBXBuildFile", out)

    def test_manifest_joins_the_asset_catalogue(self):
        out, _ = patch(PROJECT)
        group = out.split("children = (")[1].split(");")[0]
        self.assertIn(MANIFEST, group, "not added to the group")
        phase = out.split("files = (")[1].split(");")[0]
        self.assertIn(f"{MANIFEST} in Resources", phase, "not added to the resources phase")

    def test_ids_are_well_formed_and_unique(self):
        out, _ = patch(PROJECT)
        ref = out.split(f" /* {MANIFEST} */ = ")[0].split()[-1]
        build = out.split(f" /* {MANIFEST} in Resources */ = ")[0].split()[-1]
        for oid in (ref, build):
            self.assertEqual(len(oid), 24, f"{oid} is not 24 chars")
            self.assertRegex(oid, r"^[0-9A-F]{24}$")
        self.assertNotEqual(ref, build)
        self.assertNotIn(ref, PROJECT, "collides with an existing id")
        self.assertNotIn(build, PROJECT, "collides with an existing id")

    def test_running_twice_changes_nothing(self):
        once, _ = patch(PROJECT)
        twice, note = patch(once)
        self.assertEqual(once, twice)
        self.assertEqual(note, "already registered")

    def test_declarations_are_not_mistaken_for_memberships(self):
        out, _ = patch(PROJECT)
        # Exactly one declaration and one membership entry for each new object.
        self.assertEqual(out.count(f"/* {MANIFEST} */ = {{isa ="), 1)
        self.assertEqual(out.count(f"/* {MANIFEST} */,"), 1)
        self.assertEqual(out.count(f"/* {MANIFEST} in Resources */ = {{isa ="), 1)
        self.assertEqual(out.count(f"/* {MANIFEST} in Resources */,"), 1)

    def test_fails_loudly_when_the_anchor_is_gone(self):
        # Better to fail the build than ship an IPA App Store Connect will reject.
        with self.assertRaises(SystemExit):
            patch(PROJECT.replace("Assets.xcassets", "Media.xcassets"))

    def test_fails_when_anchor_is_declared_but_unused(self):
        stripped = PROJECT.replace(
            "\t\t\t\t0F1E2D3C4B5A697887665544 /* Assets.xcassets */,\n", ""
        )
        with self.assertRaises(SystemExit):
            patch(stripped)

    def test_end_to_end_through_the_cli(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "project.pbxproj"
            path.write_text(PROJECT)
            script = Path(__file__).parent / "add_privacy_manifest.py"
            result = subprocess.run(
                [sys.executable, str(script), str(path)],
                capture_output=True, text=True, check=True,
            )
            self.assertIn("added to", result.stdout)
            self.assertIn(MANIFEST, path.read_text())


if __name__ == "__main__":
    unittest.main(verbosity=2)
