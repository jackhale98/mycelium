# Android release setup

Google Play needs a signed Android App Bundle. Until the secrets below exist,
the workflow builds an unsigned debug APK instead — useful for sideloading, but
not something Play will accept.

## Create a keystore

Once, on your own machine:

```sh
keytool -genkey -v \
  -keystore mycelium-release.jks \
  -keyalg RSA -keysize 2048 -validity 10000 \
  -alias mycelium
```

**Back this file up, and keep the passwords.** Play identifies an app by its
signing key. Losing it means never shipping an update to the same listing again
— the only way back is a new listing under a new package name, which strands
every existing install. Store it somewhere you would trust with the only copy of
something irreplaceable, because that is what it is.

Enrolling in [Play App Signing](https://developer.android.com/studio/publish/app-signing#app-signing-google-play)
makes this survivable: Google holds the signing key and you keep an upload key,
which can be reset if lost. Worth doing at the point you create the listing.

## Add the secrets

The keystore is binary, so it goes in as base64:

```sh
base64 -w0 mycelium-release.jks     # Linux
base64 -i mycelium-release.jks      # macOS
```

Then under **Settings → Secrets and variables → Actions**:

| Secret | Value |
| --- | --- |
| `ANDROID_KEYSTORE` | the base64 output above |
| `ANDROID_KEYSTORE_PASSWORD` | the store password |
| `ANDROID_KEY_ALIAS` | `mycelium`, or whatever `-alias` you used |
| `ANDROID_KEY_PASSWORD` | the key password, if it differs from the store password |

The build detects `ANDROID_KEYSTORE` and switches to a signed release; nothing
else needs changing. Tag a version and the draft release gets both a
`mycelium-android.aab` for Play and a `mycelium-android-arm64.apk` for
sideloading, since Play Console takes the bundle and a browser cannot install one.

## How it is applied

`tauri android init` regenerates `app/build.gradle.kts` on every build, and what
it writes has no signing configuration at all. CI writes `keystore.properties`
from the secrets and runs `.github/scripts/add_android_signing.py`, which adds
the loader, the `signingConfigs` block and the reference from the release build
type. Neither the keystore nor the properties file is ever committed.

That script is tested against the file `tauri android init` actually produces,
captured from a runner, so a change to tauri's template fails the test rather
than silently producing an unsigned build.
