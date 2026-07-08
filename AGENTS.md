# Development Notes for Agents

This file captures the important implementation context learned while building the project. It is intentionally detailed so a future session can continue without rediscovering the same Windows/Tauri/Wi-Fi pitfalls.

Do not add local SSIDs, BSSIDs, addresses, screenshots of real network lists, or other environment-specific private data. The committed UI fixture is synthetic.

## Current scope and targets

- Primary targets: Windows 11 and macOS.
- Supported build targets:
  - `x86_64-pc-windows-msvc`
  - `aarch64-pc-windows-msvc`
  - Apple Silicon / Intel macOS hosts supported by Tauri/CoreWLAN
- UI stack: Tauri v2 + Vite + Chart.js.
- Backend stack: Rust + Windows Native Wi-Fi API via `windows` crate and macOS CoreWLAN via `objc2-core-wlan`.
- Linux is not currently in scope.

## Repository architecture

The Cargo workspace members are:

- `crates/wifi-raw`
  - Leaf contract crate shared by the abstraction layer and platform implementations.
  - Avoids a dependency cycle between `wifi-core` and platform crates.
  - Contains `RawAp` and `RawError`.
  - Contains shared channel/frequency helpers and 802.11 Information Element parsing used by Windows and macOS.
- `crates/wifi-core`
  - Public cross-platform abstraction.
  - Contains `AccessPoint`, `ScanResult`, `Band`, `ScanError`, and `WifiScanner`.
  - `create_scanner()` selects the platform implementation with `cfg`.
  - The Tauri app must depend on this crate only, not directly on `wifi-windows`.
- `crates/wifi-windows`
  - Windows implementation using Native Wi-Fi (`wlanapi.dll`) through `windows = 0.58`.
  - Parses BSS entries and 802.11 Information Elements.
- `crates/wifi-macos`
  - macOS implementation using CoreWLAN (`CWWiFiClient` / `CWInterface.scanForNetworks`).
- `src-tauri`
  - Tauri shell and command bridge.
  - Handles Windows self-elevation before building the Tauri window.
- `frontend`
  - Vite/Chart.js UI.
  - Uses real Tauri data when `window.__TAURI__` exists.
  - Uses `frontend/src/ui-fixture.json` in browser/dev mode.
- `tests`
  - Real-system abstraction E2E plus `scan-demo` utility.

## Public model details

`AccessPoint` includes:

- `id`
  - Stable identifier for UI identity.
  - Uses BSSID when available.
  - Falls back to deterministic `ssid-channel-width` when the BSSID is unavailable (fallback scan path).
  - Prefer this over pixel position or generated GUIDs. A new GUID every scan would break persistence.
- `ssid`
- `bssid`
- `band`
- `channel`
- `frequency_mhz`
  - Primary/control channel frequency.
- `center_freq_mhz`
  - True center frequency used for trapezoid placement.
  - Important for 40/80/160/320 MHz networks.
- `channel_width_mhz`
- `rssi_dbm`
- `channel_utilization`
  - QBSS utilization percentage, when advertised.
- `is_dfs`
- `security`
- `phy`

## Windows scanning details

Windows implementation flow:

1. `WlanOpenHandle`
2. `WlanEnumInterfaces`
3. Pick the first attached interface.
4. `WlanScan`
5. Wait roughly 4 seconds.
6. `WlanGetNetworkBssList`
7. Parse each `WLAN_BSS_ENTRY`.
8. If BSS scan fails or is unavailable, fall back to `WlanGetAvailableNetworkList`.

Important constraints:

- `WlanGetNetworkBssList` gives the needed per-BSSID/channel/IE data.
- It requires admin/elevation on Windows.
- Windows 11 also requires Location Services to be enabled.
- If Location Services is off, `WlanGetNetworkBssList` and `netsh wlan show networks mode=bssid` can fail with access denied / empty data even when elevated.
- `WlanGetAvailableNetworkList` is only a coarse fallback:
  - It may return SSIDs and signal quality.
  - It does not provide per-BSSID channel/band/width/congestion.
  - In fallback records, frequency/channel are `0`, width is `20`, and BSSID is empty.

Do not treat fallback data as equivalent to real BSS scan data.

## macOS scanning details

macOS implementation flow:

1. `CWWiFiClient::sharedWiFiClient`
2. Select the default `CWInterface`.
3. Require Wi-Fi power to be on.
4. `scanForNetworksWithName:includeHidden:error:`
5. Parse each `CWNetwork`.
6. Use `CWNetwork.wlanChannel` for primary channel/band/width.
7. Use `CWNetwork.informationElementData` with the shared IE parser for QBSS, width/center, security hints, and PHY hints.

Important constraints:

- macOS gates SSID/BSSID and detailed Wi-Fi scan data behind Location Services permission.
- The Tauri bundle merges `src-tauri/Info.plist`, which contains `NSLocationWhenInUseUsageDescription`.
- CoreWLAN does not currently expose a 320 MHz channel-width enum in the bindings; EHT/Wi-Fi 7 can be detected from IE extension tag 106, but width may fall back to IE/CoreWLAN data that is available.
- Invalid RSSI records (`0` or out of plausible range) are filtered so the abstraction contract keeps `rssi_dbm` negative and realistic.

## Information Element parsing

`crates/wifi-raw/src/ie.rs` parses only what the app currently needs:

- IE 11: BSS Load / QBSS.
  - `body[2]` is channel utilization, raw `0..255`.
  - The app stores `ceil(raw / 255 * 100)` as a percent.
- IE 48: RSN.
  - Used to distinguish WPA2 vs WPA3.
  - SAE AKM types (`8`, `9`) map to WPA3.
  - RSN without SAE currently maps to WPA2.
- IE 61: HT Operation.
  - Detects 40 MHz via secondary-channel offset.
  - Computes center frequency as primary +/- 10 MHz.
- IE 192: VHT Operation.
  - Detects 80/160 MHz and center frequency from CCFS0/CCFS1.
  - This was necessary because otherwise 5 GHz domes were centered on primary channels instead of true center.
- IE 255 extension tags:
  - Ext 36 => HE / 802.11ax marker.
  - Ext 106 => EHT / 802.11be marker.

Known limitations:

- 6 GHz HE Operation width/center parsing is still basic; the committed UI fixture includes synthetic 6E examples for visual integration stability.
- Security parsing is intentionally coarse. Do not overstate it as a full supplicant-grade RSN parser.

## Channel and band math

`crates/wifi-raw/src/channel.rs` contains frequency/channel helpers.

Current mapping:

- 2.4 GHz:
  - Channel 14 special-case at 2484 MHz.
  - Channels 1-13: `(freq_mhz - 2407) / 5`.
- 5 GHz:
  - `(freq_mhz - 5000) / 5`.
- 6 GHz:
  - Channel 2 special-case at 5935 MHz.
  - `(freq_mhz - 5950) / 5`.

DFS detection:

- 5 GHz channel 52 through 144.
- Implemented as `(5000..5900).contains(freq_mhz) && (52..=144).contains(channel)`.

## UI behavior

The graph uses a channel-spectrum view:

- Flat-top trapezoids, not rounded domes.
- X-axis is channel-numbered, but internally plotted in MHz for correct spacing.
- Width is shown by trapezoid base:
  - 20/40 MHz on 2.4 GHz.
  - 20/40/80/160 on 5 GHz.
  - 20/80/160/320 examples in the fixture for 6 GHz.
- Lines are outline-only. Filled translucent polygons caused ugly opaque color blocks in dense 2.4 GHz data.
- Static labels:
  - Only networks that dominate their full channel width are labeled by default.
  - "Dominates" means no stronger AP overlaps any part of that AP's width.
  - Label de-collision is by pixel position only, not SSID, so repeated SSIDs in separate clusters can both label.
- Hover behavior:
  - Snap to nearest trapezoid apex within the threshold.
  - Highlight that network with a thicker/brighter line.
  - Dim the rest.
  - Show a readable translucent name pill containing SSID and RSSI dBm.
- Do not bring back always-on labels for every network; dense 2.4 GHz becomes unreadable.

## UI fixture and screenshots

`frontend/src/ui-fixture.json` is synthetic and safe for public use.

It intentionally includes:

- Dense 2.4 GHz apartment-style overlap.
- Some 40 MHz 2.4 GHz examples.
- 5 GHz DFS examples.
- 40/80 MHz 5 GHz examples.
- Synthetic 6E/Wi-Fi 7 examples, including wide 160/320 MHz channels.

Do not replace this fixture with a real scan containing local SSIDs/BSSIDs. If a real scan is used for development, keep it under ignored `tmp/`.

Screenshots in `docs/images/` are generated from the synthetic fixture:

- `docs/images/spectrum-overview.png`
- `docs/images/snap-hover.png`

Regenerate them with:

```powershell
cd frontend
npm run shot
```

Then copy `tmp/shot.png` to the desired docs image path if needed.

## Test strategy

There are two test layers by design:

### 1. Real-system abstraction E2E

Location:

```text
tests/tests/e2e_abstraction.rs
```

Purpose:

- Exercises `wifi-core::create_scanner()`.
- On Windows, this calls the real `wifi-windows` implementation. On macOS, this calls the real `wifi-macos` CoreWLAN implementation.
- Requires the machine to have Wi-Fi hardware.
- Full scan fidelity requires Windows admin plus Location Services, or macOS Location Services permission.
- The test allows empty real scan results, but requires a non-empty interface name.
- Mock coverage ensures all bands/DFS/congestion are represented.

Run:

```powershell
cargo test -p wifi-tests --tests -- --test-threads=1
```

### 2. UI integration test

Location:

```text
frontend/e2e/ui.integration.spec.js
```

Purpose:

- Browser/Vite/Chart.js integration against `ui-fixture.json`.
- Not a full Tauri system E2E.
- This is intentional so visual iteration does not depend on local RF conditions.

Run:

```powershell
cd frontend
npm run test:ui
```

Playwright config:

- Uses system Edge via `channel: "msedge"` on Windows to match WebView2/Edge.
- Uses Playwright WebKit on macOS to match Tauri's WKWebView engine.
- Do not download bundled Chromium unless there is a strong reason.

## Fast visual iteration

Use:

```powershell
cd frontend
npm run shot
```

This runs `frontend/shot.cjs` and writes:

```text
tmp/shot.png
```

Use this for UI work:

1. Edit frontend files.
2. `npm run shot`.
3. Inspect `tmp/shot.png`.
4. Run `npm run test:ui`.

This loop was created specifically because UI density/labeling issues are easier to catch visually than through assertions.

## Build commands

Install frontend dependencies:

```powershell
npm --prefix frontend install
```

Build Rust workspace:

```powershell
cargo build --workspace
```

Run abstraction tests:

```powershell
cargo test -p wifi-tests --tests -- --test-threads=1
```

Run UI integration test:

```powershell
cd frontend
npm run test:ui
```

Production Tauri build:

```powershell
cargo install tauri-cli --locked
cargo tauri build --no-bundle
```

Important: plain `cargo build --release` is not the right way to produce the user-facing app. It can leave the app pointing at the dev URL (`localhost:5173`) instead of embedding `frontend/dist`. Use `cargo tauri build --no-bundle` or full `cargo tauri build`.

The exe is produced at:

```text
target\release\wifi-scanner-app.exe
```

## GitHub Actions macOS APP pipeline

Workflow:

```text
.github/workflows/macos-app.yml
```

It runs on `master`, `v*` tags, and manual dispatch.

What it does:

1. Checks out the repo on `macos-latest`.
2. Installs stable Rust with `aarch64-apple-darwin`.
3. Installs Node 20 and `npm ci --prefix frontend`.
4. Runs `npx --yes @tauri-apps/cli@2 build --target aarch64-apple-darwin --bundles app --no-sign --ci`.
5. Ad-hoc signs the app bundle with `codesign --force --deep --sign -`.
6. Creates `WiFiScanner-macos-arm64.dmg` from the signed app bundle, an `/Applications` shortcut, and first-run instructions.
7. Uploads the DMG as `WiFiScanner-macos-arm64-dmg`.

## GitHub Actions release pipeline

Workflow:

```text
.github/workflows/release.yml
```

It runs when either the Windows EXE or macOS APP workflow completes successfully on `master`.

What it does:

1. Looks up successful Windows and macOS workflow runs for the same commit SHA.
2. Exits without publishing if the matching platform build is not ready yet.
3. Downloads `WiFiScanner.exe` and `WiFiScanner-macos-arm64.dmg`.
4. Creates a release tagged `build-<12-char-sha>` with both files attached.

## GitHub Actions Windows EXE pipeline

Workflow:

```text
.github/workflows/windows-exe.yml
```

It runs on `master`, `v*` tags, and manual dispatch.

What it does:

1. Checks out the repo on `windows-latest`.
2. Installs stable Rust with `x86_64-pc-windows-msvc`.
3. Installs Node 20 and `npm ci --prefix frontend`.
4. Runs `npx --yes @tauri-apps/cli@2 build --no-bundle`.
6. Copies `target/release/wifi-scanner-app.exe` to `artifacts/WiFiScanner.exe`.
7. Uploads `WiFiScanner.exe` as the artifact `WiFiScanner-windows-x64-exe`.

Distribution decision:

- Do not ship MSIX right now.
- MSIX requires signing and is not the desired user experience for this project.
- A standalone `.exe` that can be downloaded and run is the intended distribution format for now.
- The app self-elevates with UAC at launch because detailed Wi-Fi scanning requires elevation.
## Windows app shell details

`src-tauri/src/main.rs` uses:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
```

Reason:

- Debug builds keep a console for logs.
- Release builds suppress the extra console window.

`src-tauri/src/lib.rs` self-elevates on Windows:

- Checks token elevation.
- If not elevated, relaunches itself with ShellExecute `runas`.
- Environment variable `WIFI_SCANNER_NOELEVATE` disables this for development/testing.

The app uses native OS window decorations:

- Keep Tauri window decorations enabled so macOS gets Cocoa chrome and Windows gets native Windows chrome.
- Do not add custom minimize/close controls in the web UI unless explicitly reintroducing a custom titlebar.

## Dependencies and why they exist

Rust workspace:

- `serde`
  - Serialize scan results from Rust to the Tauri frontend.
- `serde_json`
  - Used by `tests/scan-demo --json` to generate fixtures or inspect scans.
- `thiserror`
  - Clear typed errors for `wifi-raw` and `wifi-core`.
- `windows`
  - Native Wi-Fi API in `wifi-windows`.
  - Shell/token APIs in `src-tauri` for self-elevation.
- `objc2-core-wlan` / `objc2-foundation`
  - CoreWLAN and Foundation bindings for the macOS scanner.
- `tauri`
  - Desktop shell.
- `tauri-build`
  - Tauri build integration.

Frontend:

- `chart.js`
  - Renders the spectrum/trapezoid charts.
- `vite`
  - Frontend dev/build tool.
- `@playwright/test`
  - UI integration tests and screenshot loop.

Avoid adding dependencies unless they clearly simplify one of these core responsibilities.

## Known gotchas

- Windows Location Services must be on, or Wi-Fi scan data may be empty / access denied.
- Admin is required for detailed BSS scanning.
- macOS Location Services permission is required for SSID/BSSID scan details.
- Some APs do not advertise QBSS utilization; `channel_utilization` can be null.
- The non-admin fallback cannot provide useful channel/width/band data.
- Real RF conditions change constantly; UI tests should not depend on live scans.
- Use BSSID-based `id` for identity. Do not use pixel position. Do not use random GUIDs for scan rows.
- Keep public repo data synthetic.
