//! Tool to install software.

use anyhow::Context;

use super::RustTool;

enum PackageManager {
    Apt,
    Brew,
    Dnf,
    Pacman,
}

impl PackageManager {
    /// Get the name of the package manager.
    fn get_name(&self) -> &'static str {
        match self {
            PackageManager::Apt => "apt",
            PackageManager::Brew => "brew",
            PackageManager::Dnf => "dnf",
            PackageManager::Pacman => "pacman",
        }
    }

    /// Install a list of packages.
    fn install(&self, packages: Vec<String>) -> anyhow::Result<()> {
        let package_manager = self.get_name();
        let mut command = if let Self::Brew = self {
            std::process::Command::new(package_manager)
        } else {
            let mut command = std::process::Command::new("sudo");
            command.arg(package_manager);
            command
        };
        match self {
            PackageManager::Apt | PackageManager::Brew | PackageManager::Dnf => {
                command.arg("install");
            }
            PackageManager::Pacman => {
                command.arg("-S");
            }
        }
        command.args(packages);
        let status = command
            .status()
            .context("Failed to run the package manager")?;
        if !status.success() {
            anyhow::bail!("The package manager failed with status code {}", status);
        }
        Ok(())
    }

    /// Guess the package manager based on the host OS.
    fn guess() -> Self {
        let host_info = crate::host_info::HostInformation::new();
        match host_info.os.as_str() {
            "macos" => PackageManager::Brew,
            x => {
                if x.contains("Arch Linux") {
                    return PackageManager::Pacman;
                }
                if x.contains("Fedora") {
                    return PackageManager::Dnf;
                }
                // APT is always a decent guess
                PackageManager::Apt
            }
        }
    }
}

/// Tool to install software.
pub struct PackageManagerTool {
    /// The underlying package manager binary.
    package_manager: PackageManager,
}

/// Input to the package manager tool.
#[derive(serde::Deserialize, schemars::JsonSchema, Debug)]
pub struct PackageManagerToolInput {
    /// The names of packages to suggest that the user install.
    /// The user will be presented with a prompt where they can choose whether or not to the install the packages.
    /// If the user chooses to install the packages, the packages will be installed using the system's package manager.
    /// Make sure the package names are appropriate for the system's OS or distribution.
    packages_to_install: Vec<String>,
}

impl PackageManagerTool {
    pub fn new() -> Self {
        PackageManagerTool {
            package_manager: PackageManager::guess(),
        }
    }
}

impl RustTool for PackageManagerTool {
    type Input = PackageManagerToolInput;

    fn get_name(&self) -> String {
        "package_manager".to_string()
    }

    fn get_description(&self) -> String {
        format!(
            "Install packages on the user's system using the `{}` command.",
            self.package_manager.get_name()
        )
    }

    async fn run(self: std::sync::Arc<Self>, input: Self::Input) -> anyhow::Result<String> {
        let packages = input.packages_to_install.clone();
        self.package_manager.install(packages)?;
        Ok("Finished".to_string())
    }
}
