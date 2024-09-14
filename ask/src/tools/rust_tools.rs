use std::sync::Arc;

/// This function exposes all the statically registered Rust tools.
pub fn get_rust_tools() -> Vec<Arc<dyn super::Tool>> {
    vec![
        Arc::new(super::cloud_context::CloudContextTool),
        Arc::new(super::filesystem::ReadFilesTool),
        Arc::new(super::filesystem::WriteFilesTool),
        Arc::new(super::http_request::HttpGetTool),
        Arc::new(super::kubernetes::ArgocdStatusTool),
        Arc::new(super::open::OpenTool),
        Arc::new(super::package_manager::PackageManagerTool::new()),
        Arc::new(super::software_versions::SoftwareVersionsTool::new()),
        Arc::new(super::terraform::TerraformPlanTool),
        Arc::new(super::binary_tool::BinaryTool::new_without_output(
            "ffmpeg",
            "Run `ffmpeg` - a CLI tool for video processing - with the provided arguments.",
            true,
        )),
        Arc::new(super::binary_tool::BinaryTool::new_with_output(
            "tar",
            "Run `tar` - a CLI tool to compress and decompress tar files - with the provided arguments.",
            false,
        )),
        Arc::new(super::binary_tool::BinaryTool::new_with_output(
            "unzip",
            "Run `unzip` - a CLI tool to decompress zip files - with the provided arguments.",
            false,
        )),
    ]
}
