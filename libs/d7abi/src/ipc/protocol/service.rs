use alloc::string::String;
use core::fmt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ServiceName(pub String);
impl fmt::Display for ServiceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registration {
    /// Service name
    pub name: ServiceName,
    /// Oneshot services are considired running after they have completed successfully
    #[serde(default)]
    pub oneshot: bool,
}
