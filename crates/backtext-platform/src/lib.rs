pub mod simulator;
pub mod traits;

#[cfg(target_os = "macos")]
pub mod macos;

pub use simulator::SimulatorPlatformSensors;
pub use traits::*;

#[cfg(target_os = "macos")]
pub use sha2;

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
