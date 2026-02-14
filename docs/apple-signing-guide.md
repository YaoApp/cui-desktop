# Apple Code Signing & Notarization Guide

This guide walks through obtaining and configuring all Apple certificates and credentials needed to sign and notarize the macOS build in CI.

## Prerequisites

- Apple Developer Program membership ($99/year): https://developer.apple.com/programs/
- Xcode installed on your Mac
- Access to https://developer.apple.com/account

---

## Step 1: Create a Developer ID Application Certificate

This certificate is used to sign apps distributed **outside the Mac App Store**.

1. Open **Keychain Access** on your Mac
2. Menu → **Keychain Access** → **Certificate Assistant** → **Request a Certificate From a Certificate Authority**
   - User Email: your Apple ID email
   - Common Name: your name
   - CA Email: leave empty
   - Request is: **Saved to disk**
   - Save the `.certSigningRequest` file
3. Go to https://developer.apple.com/account/resources/certificates/list
4. Click **+** to create a new certificate
5. Select **Developer ID Application** → Continue
6. Upload the `.certSigningRequest` file from step 2
7. Download the generated `.cer` file (e.g. `developerID_application.cer`)
8. Double-click to install it into Keychain Access

## Step 2: Export the Private Key (.p12)

1. Open **Keychain Access**
2. In **My Certificates**, find the certificate named like:
   ```
   Developer ID Application: Your Name (TEAMID)
   ```
3. Expand it (click the triangle) — you should see a **private key** underneath
4. Right-click the **private key** → **Export**
5. Save as `.p12` format, set a password — remember this password (this is `APPLE_PRIVATE_KEY_PASSWORD`)

## Step 3: Download the Developer ID G2 CA Certificate

1. Go to https://www.apple.com/certificateauthority/
2. Download **Developer ID - G2** (DER format):
   https://www.apple.com/certificateauthority/DeveloperIDG2CA.cer
3. Save as `DeveloperIDG2CA.cer`

## Step 4: Create an App-Specific Password

Apple requires an app-specific password for notarization (not your regular Apple ID password).

1. Go to https://appleid.apple.com/account/manage
2. Sign in with your Apple ID
3. In the **Sign-In and Security** section, click **App-Specific Passwords**
4. Click **Generate an app-specific password**
5. Label it: `cui-desktop-notarize`
6. Copy the generated password (format: `xxxx-xxxx-xxxx-xxxx`) — this is `APPLE_APP_SPEC_PASS`

## Step 5: Find Your Team ID

1. Go to https://developer.apple.com/account#MembershipDetailsCard
2. Your **Team ID** is displayed (10-character alphanumeric string, e.g. `ABC1234DEF`)

## Step 6: Find Your Signing Identity

Run this command in Terminal:

```bash
security find-identity -v -p codesigning
```

Look for a line like:
```
1) ABCDEF1234567890... "Developer ID Application: Your Name (TEAMID)"
```

The full string in quotes is your signing identity. The part after the colon is what goes into `APPLE_SIGN`:
```
Your Name (TEAMID)
```

For example, if the output is `"Developer ID Application: Yao Team (ABC1234DEF)"`, then `APPLE_SIGN` = `Yao Team (ABC1234DEF)`.

---

## Step 7: Base64 Encode the Certificates

The CI needs certificates as base64-encoded strings stored in GitHub Secrets.

```bash
# Developer ID G2 CA certificate
base64 -i DeveloperIDG2CA.cer | pbcopy
# Paste into GitHub Secret: APPLE_DEVELOPERIDG2CA

# Distribution certificate (.cer exported from step 1)
base64 -i developerID_application.cer | pbcopy
# Paste into GitHub Secret: APPLE_DISTRIBUTION

# Private key (.p12 exported from step 2)
base64 -i private_key.p12 | pbcopy
# Paste into GitHub Secret: APPLE_PRIVATE_KEY
```

---

## Step 8: Configure GitHub Secrets

Go to **GitHub → Repository Settings → Secrets and variables → Actions → New repository secret**

Or if using org-level secrets: **GitHub → Organization Settings → Secrets and variables → Actions**

Add these secrets:

| Secret Name | Value | Source |
|---|---|---|
| `APPLE_DEVELOPERIDG2CA` | Base64 of `DeveloperIDG2CA.cer` | Step 3 + Step 7 |
| `APPLE_DISTRIBUTION` | Base64 of `developerID_application.cer` | Step 1 + Step 7 |
| `APPLE_PRIVATE_KEY` | Base64 of `private_key.p12` | Step 2 + Step 7 |
| `APPLE_PRIVATE_KEY_PASSWORD` | Password you set when exporting .p12 | Step 2 |
| `KEYCHAIN_PASSWORD` | Any random string (e.g. `ci-keychain-2024`) | You choose |
| `APPLE_SIGN` | Your signing identity (e.g. `Yao Team (ABC1234DEF)`) | Step 6 |
| `APPLE_ID` | Your Apple ID email | Your Apple account |
| `APPLE_TEAME_ID` | Your Team ID (e.g. `ABC1234DEF`) | Step 5 |
| `APPLE_APP_SPEC_PASS` | App-specific password (`xxxx-xxxx-xxxx-xxxx`) | Step 4 |

---

## Verify Locally (Optional)

Before pushing to CI, you can test signing locally:

```bash
# Build the app
cargo tauri build

# Sign manually
codesign --deep --force --verbose --timestamp \
  --sign "Developer ID Application: Your Name (TEAMID)" \
  "src-tauri/target/release/bundle/macos/Yao Agents.app"

# Verify signature
codesign --verify --deep --strict --verbose=2 \
  "src-tauri/target/release/bundle/macos/Yao Agents.app"

# Notarize (zip first, then submit)
zip -r YaoAgents.zip "src-tauri/target/release/bundle/macos/Yao Agents.app"
xcrun notarytool submit YaoAgents.zip \
  --apple-id "your@email.com" \
  --team-id "ABC1234DEF" \
  --password "xxxx-xxxx-xxxx-xxxx" \
  --wait

# Staple the notarization ticket
xcrun stapler staple "src-tauri/target/release/bundle/macos/Yao Agents.app"
```

---

## Quick Check: Do I Already Have Everything?

If you've already set up signing for the `yao/yao` repository, you likely have all secrets already. Check:

```bash
# List your signing identities
security find-identity -v -p codesigning

# If you see "Developer ID Application: ..." you're good
```

Then just make sure the same secrets are accessible to the `cui-desktop` repository (either as org-level secrets or copied to the repo).

---

## Troubleshooting

| Problem | Solution |
|---|---|
| `errSecInternalComponent` | Add `security set-key-partition-list` after import (already in CI) |
| `The signature of the binary is invalid` | Ensure `--deep --force --timestamp` flags are used |
| Notarization rejected | Check `xcrun notarytool log <submission-id>` for details |
| `Developer ID Application` cert not found | Re-create at developer.apple.com/account |
| App-specific password invalid | Generate a new one at appleid.apple.com |
