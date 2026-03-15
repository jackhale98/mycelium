# iOS TestFlight Setup (without a Mac)

This guide covers generating Apple signing credentials on Linux, configuring
GitHub secrets, and letting the CI workflow build, sign, and upload Mycelium
to TestFlight automatically.

## Prerequisites

- Apple Developer Program membership ($99/year) — https://developer.apple.com/programs/
- OpenSSL installed (`sudo apt install openssl` on Ubuntu)

## Step 1: Generate a Distribution Certificate

```bash
# Generate a private key
openssl genrsa -out ios_dist.key 2048

# Create a Certificate Signing Request (CSR)
openssl req -new -key ios_dist.key -out ios_dist.csr \
  -subj "/CN=Your Name/emailAddress=your@email.com"
```

## Step 2: Upload CSR to Apple Developer Portal

1. Go to https://developer.apple.com/account/resources/certificates/add
2. Select **Apple Distribution**
3. Upload `ios_dist.csr`
4. Download the resulting `distribution.cer`

## Step 3: Create a .p12 Bundle

**Important:** Use the `-legacy` flag so the .p12 is compatible with the macOS
`security` framework on GitHub Actions runners (OpenSSL 3.x changed the default
encryption and without `-legacy` you get "MAC verification failed" errors).

```bash
# Convert Apple's .cer (DER format) to PEM
openssl x509 -inform DER -in distribution.cer -out distribution.pem

# Bundle into .p12 — note the -legacy flag!
openssl pkcs12 -export -legacy -out distribution.p12 \
  -inkey ios_dist.key -in distribution.pem \
  -password pass:YOUR_P12_PASSWORD
```

## Step 4: Register an App ID

1. Go to https://developer.apple.com/account/resources/identifiers/add/bundleId
2. Select **App IDs** → **App**
3. Description: `Mycelium`
4. Bundle ID (Explicit): `com.mycelium.app`
   - Must match the `identifier` field in `src-tauri/tauri.conf.json`
5. No extra capabilities needed — click **Register**

## Step 5: Create a Provisioning Profile

1. Go to https://developer.apple.com/account/resources/profiles/add
2. Select **App Store Connect** (this covers TestFlight)
3. Select the App ID you just created
4. Select the distribution certificate from Step 2
5. Name it `Mycelium Distribution`
6. Download the `.mobileprovision` file

## Step 6: Add GitHub Secrets

1. Go to your GitHub repo → Settings → Environments
2. Create an environment called `ios-release`
3. Add these secrets:

```bash
# Generate base64 values on your Linux machine:
base64 -w 0 distribution.p12        # → APPLE_CERTIFICATE
base64 -w 0 profile.mobileprovision  # → APPLE_PROVISIONING_PROFILE
```

| Secret Name | Value |
|------------|-------|
| `APPLE_CERTIFICATE` | base64-encoded contents of `distribution.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | The password you set in Step 3 |
| `APPLE_TEAM_ID` | Your 10-character Team ID (found at developer.apple.com → Membership Details) |
| `APPLE_PROVISIONING_PROFILE` | base64-encoded contents of the `.mobileprovision` file |

## Step 7: Create the App in App Store Connect

1. Go to https://appstoreconnect.apple.com → My Apps → "+"
2. New App → iOS
3. Name: `Mycelium`
4. Bundle ID: select the one from Step 4
5. SKU: `mycelium`
6. Save — this creates the TestFlight landing page

## Step 8: Create an App Store Connect API Key (for TestFlight upload)

1. Go to https://appstoreconnect.apple.com → Users and Access → Integrations → App Store Connect API
2. Generate a new key with **App Manager** role
3. Download the `.p8` file and note the Key ID and Issuer ID
4. Base64-encode the key: `base64 -w 0 AuthKey_XXXXXXXX.p8`

Add these secrets to the same `ios-release` environment:

| Secret Name | Value |
|------------|-------|
| `APP_STORE_CONNECT_API_KEY` | base64-encoded `.p8` file |
| `APP_STORE_CONNECT_KEY_ID` | the Key ID shown in App Store Connect |
| `APP_STORE_CONNECT_ISSUER_ID` | the Issuer ID shown at the top of the API Keys page |

If the API key secrets are not configured, the workflow still builds the `.ipa`
and uploads it as a GitHub artifact — you can then upload manually using the
Transporter app on a Mac.

## Step 9: Trigger a Build

Push a new version tag to trigger the iOS build workflow:

```bash
git tag v0.X.X
git push origin v0.X.X
```

You can also trigger the build manually from the Actions tab using
**workflow_dispatch** (no tag required).

## Step 10: Verify the Build

1. Check the GitHub Actions run — the "Upload to TestFlight" step should show
   `UPLOAD SUCCEEDED` with a Delivery UUID
2. Go to https://appstoreconnect.apple.com → My Apps → Mycelium → TestFlight
3. The build appears after Apple processes it (usually 15-30 minutes)
4. Apple may email you about export compliance — for standard apps with no
   custom encryption, answer "No"
5. Once processed, add internal/external testers — they'll get a TestFlight
   notification on their device

## How the Workflow Works

The CI workflow (`.github/workflows/build-ios.yml`) uses **Tauri's native
signing support** rather than manual certificate installation.

### Key environment variables

The workflow maps your GitHub secrets to Tauri's expected env var names:

| GitHub Secret | Env Var Set in Workflow | Purpose |
|---|---|---|
| `APPLE_TEAM_ID` | `APPLE_DEVELOPMENT_TEAM` | Configures the Xcode project team ID |
| `APPLE_CERTIFICATE` | `IOS_CERTIFICATE` | Base64 .p12 — Tauri creates a keychain and imports it |
| `APPLE_CERTIFICATE_PASSWORD` | `IOS_CERTIFICATE_PASSWORD` | Unlocks the .p12 during import |
| `APPLE_PROVISIONING_PROFILE` | `IOS_MOBILE_PROVISION` | Base64 .mobileprovision — Tauri installs it with correct UUID filename |
| `APP_STORE_CONNECT_KEY_ID` | `APPLE_API_KEY` | API key ID for export/upload |
| `APP_STORE_CONNECT_ISSUER_ID` | `APPLE_API_ISSUER` | Issuer ID for export/upload |

### What Tauri handles automatically

When these env vars are set, `npx tauri ios build --export-method app-store-connect`:

1. Creates a temporary keychain and imports the certificate
2. Installs the provisioning profile with the correct UUID-based filename
3. Patches the Xcode project (`project.pbxproj`) with manual signing settings:
   `CODE_SIGN_STYLE=Manual`, `DEVELOPMENT_TEAM`, `CODE_SIGN_IDENTITY`,
   `PROVISIONING_PROFILE_SPECIFIER`
4. Builds, archives, and exports a signed `.ipa`

**`APPLE_DEVELOPMENT_TEAM` must be set on both `tauri ios init` and
`tauri ios build`** — the init step generates the Xcode project and needs the
team ID to set up the project correctly.

### Critical code requirements

The Rust `lib.rs` must include the mobile entry point macro:

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // ...
}
```

Without this, the compiled library is missing required runtime symbols and the
build fails with: "Library does not include required runtime symbols."

The `Cargo.toml` must include the correct crate types:

```toml
[lib]
name = "mycelium_lib"
crate-type = ["lib", "cdylib", "staticlib"]
```

`staticlib` is required for iOS (static linking).

### Runner requirements

- **macOS 15** runner (`runs-on: macos-15`) — provides Xcode 16.4, which
  supports the Xcode project format 77 generated by Tauri CLI 2.10+
- Earlier runners (macOS 14 with Xcode 15.x) cannot open format-77 projects
- `@tauri-apps/cli` must be in `devDependencies` (not just globally installed)

### Package.json requirements

```json
{
  "scripts": {
    "tauri": "tauri"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2"
  }
}
```

The `"tauri": "tauri"` script is required by `tauri-action` for desktop builds.
The `@tauri-apps/cli` devDependency provides the `tauri` binary for `npx tauri`.

### iOS-specific considerations

**App icon**: `icon.png` in `src-tauri/icons/` must be 1024x1024 pixels. This is
the source used by Tauri to generate all required iOS icon sizes. If the icon is
smaller, App Store Connect will reject the build or show a blank icon.

**Info.plist patching**: The workflow patches the generated `Info.plist` after
`tauri ios init` to add document picker keys:
- `UIFileSharingEnabled` — enables Files app access
- `LSSupportsOpeningDocumentsInPlace` — enables "Open in Place"
- `UISupportsDocumentBrowser` — enables document browser UI

**Safe area insets**: Use `env(safe-area-inset-top)` directly in inline styles,
not through CSS custom properties. Tauri's iOS webview doesn't propagate `env()`
through `var()` references. Pattern:
```css
style="padding-top: calc(env(safe-area-inset-top, 0px) + 8px)"
```

**Folder picker**: `tauri-plugin-dialog` does NOT support `directory: true` on
iOS. Use the file picker (`directory: false`) to let users select a `.org` file,
then derive the vault folder from its parent path. This presents the native iOS
picker UI (same as iA Writer, Obsidian, etc.).

**Keyboard toolbar**: Use the `window.visualViewport` API to detect keyboard
height and position toolbars above it:
```javascript
const vv = window.visualViewport;
vv.addEventListener('resize', () => {
  const keyboardHeight = window.innerHeight - vv.height - vv.offsetTop;
  // Position toolbar at bottom: keyboardHeight px from bottom
});
```

## Troubleshooting

| Error | Cause | Fix |
|---|---|---|
| `MAC verification failed during PKCS12 import` | .p12 created with OpenSSL 3.x without `-legacy` flag | Recreate with `openssl pkcs12 -export -legacy ...` |
| `future Xcode project file format (77)` | Runner has Xcode < 16.3 | Use `runs-on: macos-15` (provides Xcode 16.4) |
| `No profiles for 'com.mycelium.app' were found` | Profile not installed or bundle ID mismatch | Verify bundle ID in provisioning profile matches `identifier` in `tauri.conf.json` |
| `Signing requires a development team` | `APPLE_DEVELOPMENT_TEAM` not set during `tauri ios init` | Ensure the env var is set on the init step, not just the build step |
| `Library does not include required runtime symbols` | Missing `#[cfg_attr(mobile, tauri::mobile_entry_point)]` | Add the macro to `src-tauri/src/lib.rs` on the `run()` function |
| `failed to read missing addr file` | Using raw `xcodebuild` instead of Tauri CLI | Always use `npx tauri ios build`, not `xcodebuild` directly |
| `npm error could not determine executable to run` | `@tauri-apps/cli` not in devDependencies | Run `npm install --save-dev @tauri-apps/cli@^2` |
| `Missing script: "tauri"` | No `tauri` script in package.json | Add `"tauri": "tauri"` to scripts |
| `cp: ../ios-plugins/...: No such file or directory` | Swift plugin path wrong relative to gen/apple/ | Use absolute path or remove step if plugin not needed |
| Blank icon in App Store Connect | `icon.png` smaller than 1024x1024 | Regenerate: `rsvg-convert -w 1024 -h 1024 app-image.svg -o icon.png` |
| Folder picker doesn't open on iOS | `directory: true` not supported by dialog plugin on iOS | Use file picker with `directory: false`, derive folder from file path |
| Header overlaps notch | Using `var(--safe-area-top)` CSS variable | Use `env(safe-area-inset-top, 0px)` directly in inline styles |
| Toolbar hidden behind keyboard | Toolbar in document flow, not fixed | Use `visualViewport` API to detect keyboard and position:fixed the toolbar |

## Security Notes

- Keep `ios_dist.key` secure — it's your private signing key
- Never commit `.p12`, `.mobileprovision`, `.p8`, or `.key` files to the repo
- Store them somewhere safe in case you need to regenerate secrets
- The `.p8` API key never expires — revoke it in App Store Connect if compromised
