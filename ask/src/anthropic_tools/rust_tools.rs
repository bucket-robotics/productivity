use std::sync::Arc;

/// This function exposes all the statically registered Rust tools.
pub fn get_rust_tools() -> Vec<Arc<dyn super::Tool>> {
    vec![
        Arc::new(super::filesystem::ReadFilesTool),
        Arc::new(super::filesystem::WriteFilesTool),
        Arc::new(super::http_request::HttpGetTool),
        Arc::new(super::open::OpenTool),
        Arc::new(super::package_manager::PackageManagerTool::new()),
        Arc::new(super::software_versions::SoftwareVersionsTool::new()),
        Arc::new(super::binary_tool::BinaryTool::new_without_output(
            "ffmpeg",
            "Run `ffmpeg` - a CLI tool for video processing - with the provided arguments.",
            true,
        )),
        Arc::new(super::binary_tool::BinaryTool::new_with_output(
            "tar",
            "Run `tar` with the provided arguments.",
            false,
        )),
        Arc::new(super::binary_tool::BinaryTool::new_with_output(
            "unzip",
            "Run `unzip` with the provided arguments.",
            false,
        )),
    ]
}
