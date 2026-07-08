//! wifi-core: cross-platform WiFi scanning abstraction.
//!
//! Consumers (e.g. the Tauri app) depend ONLY on this crate. Platform
//! implementations live in `wifi-windows` / `wifi-macos` and are selected here
//! via `cfg`. They return raw data; conversion to the public model lives here.

mod model;
mod scanner;
mod mock;

pub use mock::band_summary;
pub use model::{AccessPoint, Band, ScanError, ScanResult};
pub use scanner::{create_scanner, request_platform_permissions, WifiScanner};
