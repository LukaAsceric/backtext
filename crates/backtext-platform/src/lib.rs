pub mod ics;
pub mod simulator;
pub mod traits;

#[cfg(target_os = "macos")]
pub mod macos;

pub use ics::{fetch_and_parse_ics_url, parse_ics_events};
pub use simulator::SimulatorPlatformSensors;
pub use traits::*;

use std::sync::Arc;

pub fn default_platform_sensors() -> Arc<dyn PlatformSensors> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(macos::MacOsPlatformSensors::new())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Arc::new(SimulatorPlatformSensors::new())
    }
}
