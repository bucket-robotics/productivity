/// Information about the host machine.
pub struct HostInformation {
    /// The operating system or distribution.
    pub os: String,
    /// The architecture.
    pub architecture: String,
}

impl Default for HostInformation {
    fn default() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            architecture: std::env::consts::ARCH.to_string(),
        }
    }
}

impl HostInformation {
    /// Create a new instance of `HostInformation`.
    pub fn new() -> &'static Self {
        static HOST_INFO: std::sync::OnceLock<HostInformation> = std::sync::OnceLock::new();
        HOST_INFO.get_or_init(Self::initialize)
    }

    fn initialize() -> Self {
        let mut host_info = HostInformation::default();
        if cfg!(target_os = "linux") {
            // Run hostnamectl to get the OS and architecture.
            if let Some(output) = std::process::Command::new("hostnamectl")
                .arg("--json=short")
                .stdout(std::process::Stdio::piped())
                .output()
                .ok()
                .and_then(|x| {
                    serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&x.stdout)
                        .ok()
                })
            {
                if let Some(os) = output
                    .get("OperatingSystemPrettyName")
                    .and_then(|x| x.as_str())
                {
                    host_info.os = os.to_string();
                }
            } else if let Ok(distro) = std::fs::read_to_string("/etc/os-release") {
                if let Some(name) = distro.lines().find(|line| line.starts_with("NAME=")) {
                    host_info.os = name
                        .trim_start_matches("NAME=")
                        .trim_matches('"')
                        .to_string();
                }
            }
        }
        host_info
    }
}
