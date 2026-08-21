#!/usr/bin/env python3
"""Tests for add_android_signing.py, run by CI.

GENERATED is app/build.gradle.kts exactly as `tauri android init` produced it,
captured from a build runner — the point being that the patcher is exercised
against what tauri actually writes rather than an idea of it.
"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from add_android_signing import patch  # noqa: E402

GENERATED = '''import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

android {
    compileSdk = 36
    namespace = "com.mycelium.app"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "com.mycelium.app"
        minSdk = 24
        targetSdk = 36
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
        }
        getByName("release") {
            isMinifyEnabled = true
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
        }
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
}

rust {
    rootDirRel = "../../../"
}
'''


class TestPatch(unittest.TestCase):
    def test_adds_all_three_pieces(self):
        out = patch(GENERATED)
        self.assertIn("val keystoreProperties = Properties()", out)
        self.assertIn('signingConfigs {', out)
        self.assertIn('signingConfig = signingConfigs.getByName("release")', out)

    def test_signing_configs_sit_inside_android(self):
        out = patch(GENERATED)
        android = out.index("\nandroid {")
        rust = out.index("\nrust {")
        self.assertGreater(out.index("    signingConfigs {"), android)
        self.assertLess(out.index("    signingConfigs {"), rust)

    def test_signing_configs_come_before_build_types(self):
        # Gradle resolves signingConfigs.getByName at configuration time, so the
        # block has to be declared before the build type that references it.
        out = patch(GENERATED)
        self.assertLess(out.index("    signingConfigs {"), out.index("    buildTypes {"))

    def test_only_the_release_type_is_signed(self):
        out = patch(GENERATED)
        release = out.index('getByName("release")')
        debug = out.index('getByName("debug")')
        signing = out.index("signingConfig = signingConfigs")
        self.assertGreater(signing, release, "signing landed outside the release block")
        self.assertGreater(release, debug, "release block moved")

    def test_loader_follows_the_existing_properties_block(self):
        out = patch(GENERATED)
        self.assertLess(out.index("val tauriProperties"), out.index("val keystoreProperties"))
        # It must be top level, not swallowed into android {}.
        self.assertLess(out.index("val keystoreProperties"), out.index("\nandroid {"))

    def test_running_twice_changes_nothing(self):
        once = patch(GENERATED)
        self.assertEqual(once, patch(once))

    def test_fails_loudly_when_tauri_changes_the_template(self):
        # Better a failed build than an unsigned release that Play rejects.
        for missing in ["val tauriProperties = Properties().apply {", "    buildTypes {"]:
            with self.assertRaises(SystemExit):
                patch(GENERATED.replace(missing, "// gone"))

    def test_braces_stay_balanced(self):
        out = patch(GENERATED)
        self.assertEqual(out.count("{"), out.count("}"))


if __name__ == "__main__":
    unittest.main(verbosity=2)
