#!/usr/bin/env python3
"""Add release signing to the generated Android app module.

`tauri android init` regenerates app/build.gradle.kts on every build, and what
it writes has no signingConfigs at all — the release build type sets minification
and proguard and stops there. An unsigned release cannot be uploaded to Play, so
the configuration is added back each time.

Credentials come from keystore.properties, which CI writes from repository
secrets and which is never committed. The loader mirrors the tauriProperties
block already in the file, so the result reads like the rest of it rather than
like something bolted on.
"""

import re
import sys

LOADER = '''
val keystoreProperties = Properties().apply {
    // Written by CI from repository secrets; absent for a local or unsigned build.
    val propFile = rootProject.file("keystore.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}
'''

SIGNING_CONFIGS = '''    signingConfigs {
        create("release") {
            // Guarded so a build without the properties file still configures,
            // and fails at signing time with a clear message rather than here.
            val storePath = keystoreProperties.getProperty("storeFile")
            if (storePath != null) {
                storeFile = file(storePath)
                storePassword = keystoreProperties.getProperty("storePassword")
                keyAlias = keystoreProperties.getProperty("keyAlias")
                keyPassword = keystoreProperties.getProperty("keyPassword")
            }
        }
    }
'''

SIGNING_LINE = '            signingConfig = signingConfigs.getByName("release")\n'


def patch(source: str) -> str:
    if "signingConfigs {" in source:
        return source

    # 1. The credentials loader, right after the one tauri already writes.
    loader_anchor = re.search(r"val tauriProperties = Properties\(\)\.apply \{.*?\n\}\n", source, re.S)
    if not loader_anchor:
        raise SystemExit("error: tauriProperties block not found; cannot place the keystore loader")
    source = source.replace(loader_anchor.group(0), loader_anchor.group(0) + LOADER, 1)

    # 2. signingConfigs must sit inside android {}, before buildTypes.
    if "\n    buildTypes {\n" not in source:
        raise SystemExit("error: buildTypes block not found; cannot place signingConfigs")
    source = source.replace("\n    buildTypes {\n", "\n" + SIGNING_CONFIGS + "    buildTypes {\n", 1)

    # 3. Point the release build type at it.
    release = re.search(r'( *)getByName\("release"\) \{\n', source)
    if not release:
        raise SystemExit("error: release build type not found; cannot attach the signing config")
    source = source.replace(release.group(0), release.group(0) + SIGNING_LINE, 1)

    return source


def main() -> None:
    path = sys.argv[1]
    with open(path, encoding="utf-8") as handle:
        source = handle.read()
    patched = patch(source)
    if patched == source:
        print("release signing: already configured")
        return
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(patched)
    print("release signing: configured")


if __name__ == "__main__":
    main()
