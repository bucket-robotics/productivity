//! Tool to read files.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use anyhow::Context;

use super::RustTool;

/// Tool to read data from files and to get the contents of directories.
pub struct ReadFilesTool;

/// Input to the read files tool.
#[derive(serde::Deserialize, schemars::JsonSchema, Debug)]
pub struct ReadFilesToolInput {
    /// A mapping where the key is the filesystem path to read and the value is a short justification for why you want to read that file or directory.
    paths_to_reason_mapping: HashMap<String, String>,
}

/// A type to store access remembered permissions.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct AccessCache {
    /// The set of paths which the user has previously allowed read access to.
    allowed_read_paths: HashSet<String>,
}

impl RustTool for ReadFilesTool {
    type Input = ReadFilesToolInput;

    fn get_name(&self) -> String {
        "read_files".to_string()
    }

    fn get_description(&self) -> String {
        r" Read the content of one or more file paths from the user's computer.
            The user will be prompted to accept or deny the read request to preserve their privacy.
            For paths that are files the tool will return the content of the files as a XML `file` tags with a `path` attribute to determine which file the content is from.
            For paths that are directories the tool will return a list of the files and subdirectories in the directory using XML `directory` tags with a `path` attribute to determine which directory the content is from.
        "
        .trim()
        .to_string()
    }

    async fn run(self: std::sync::Arc<Self>, input: Self::Input) -> anyhow::Result<String> {
        let access_cache_path = super::get_cache_dir_for_tool(self.as_ref())?.join("access.json");
        let mut access_cache: AccessCache = AccessCache {
            allowed_read_paths: HashSet::with_capacity(2),
        };

        if let Ok(file) = std::fs::File::open(&access_cache_path) {
            match serde_json::from_reader(file) {
                Ok(cache) => access_cache = cache,
                Err(e) => {
                    tracing::warn!(
                        "Failed to read access cache at {}: {}",
                        access_cache_path.display(),
                        e
                    );
                }
            }
        } else {
            tracing::info!("No access cache found at {}", access_cache_path.display());
        }

        let current_dir = std::env::current_dir().context("Failed to get the current directory")?;
        let mut paths = Vec::with_capacity(input.paths_to_reason_mapping.len());
        let mut files_to_read = Vec::with_capacity(input.paths_to_reason_mapping.len());
        let mut select_choices: Vec<_> = Vec::with_capacity(input.paths_to_reason_mapping.len());
        let mut response = String::with_capacity(128);
        for (path, reason) in &input.paths_to_reason_mapping {
            let full_path = current_dir.join(path).to_string_lossy().to_string();
            select_choices.push(format!("{} - {reason}", &full_path));
            paths.push(full_path);
        }

        if paths
            .iter()
            .all(|path| access_cache.allowed_read_paths.contains(path))
        {
            // All paths have been previously allowed
            files_to_read.extend(paths);
        } else {
            // Not all paths have been previously allowed so prompt the user
            let mut disallowed_files: HashSet<_> = paths.iter().cloned().collect();
            // Create a default selection of all paths (all allowed by default
            let defaults: Vec<bool> = paths.iter().map(|_| true).collect();
            let selection = dialoguer::MultiSelect::new()
                .with_prompt("Files to allow the LLM to read:")
                .items(&select_choices)
                .defaults(&defaults)
                .interact()
                .context("From multi-select")?;

            for file_index in selection {
                let path = paths[file_index].clone();
                disallowed_files.remove(&path);
                access_cache.allowed_read_paths.insert(path.clone());
                files_to_read.push(path);
            }

            for file in disallowed_files {
                response.push_str(&format!(
                    "The user chose not to allow you to read {file}.\n"
                ));
                access_cache.allowed_read_paths.remove(&file);
            }
        }

        for file_path_string in files_to_read {
            let file_path = Path::new(&file_path_string);
            let read_result = if file_path.is_dir() {
                // Attempt to read the directory
                std::fs::read_dir(file_path)
                    .and_then(|dir| {
                        dir.map(|entry| {
                            entry.map(|entry| {
                                let path = entry.path();
                                if path.is_dir() {
                                    format!("{}/", path.display())
                                } else {
                                    format!("{}", path.display())
                                }
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()
                    })
                    .map(|result_vec| {
                        format!(
                            "<directory path=\"{}\">\n{}\n</directory>\n",
                            file_path.display(),
                            result_vec.join("\n")
                        )
                    })
            } else {
                // Attempt to read the file
                std::fs::read_to_string(file_path).map(|content| {
                    format!("<file path=\"{}\">{content}</file>\n", file_path.display())
                })
            };

            match read_result {
                Ok(content) => {
                    response.push_str(&content);
                }
                Err(e) => {
                    response.push_str(&format!("Could not read {}: {e}\n", file_path.display()));
                }
            }
        }

        // Write the access cache back to disk
        let access_cache_file = std::fs::File::create(&access_cache_path).with_context(|| {
            format!(
                "Failed to create the access cache file at {}",
                access_cache_path.display()
            )
        })?;
        serde_json::to_writer(access_cache_file, &access_cache).with_context(|| {
            format!(
                "Failed to write the access cache file at {}",
                access_cache_path.display()
            )
        })?;

        Ok(response)
    }
}

/// Tool to write data to files.
pub struct WriteFilesTool;

/// Input to the write files tool.
#[derive(serde::Deserialize, schemars::JsonSchema, Debug)]
pub struct WriteFilesInput {
    /// A mapping where the key is the filesystem path to write and the value is the content to write to that file.
    paths_to_content: HashMap<String, String>,
}

impl RustTool for WriteFilesTool {
    type Input = WriteFilesInput;

    fn get_name(&self) -> String {
        "write_files".to_string()
    }

    fn get_description(&self) -> String {
        r" Write to one or more file paths on the user's computer.
            The user will be prompted to accept or deny each write.
            Intermediate directories will be created if they don't exist.
        "
        .trim()
        .to_string()
    }

    async fn run(self: std::sync::Arc<Self>, input: Self::Input) -> anyhow::Result<String> {
        let mut response = vec![];
        let cwd = std::env::current_dir().context("Failed to get the current directory")?;
        for (file, content) in input.paths_to_content {
            let file_path = cwd.join(file);
            // TODO show the file content being written and show a diff if the file already exists
            let allow_write = dialoguer::Confirm::new()
                .with_prompt(format!("Write to {}?", file_path.display()))
                .interact()
                .context("From confirm")?;
            if !allow_write {
                response.push(format!(
                    "The user said they did not want to you to write to {}",
                    file_path.display()
                ));
                continue;
            }

            let parent_dir = file_path
                .parent()
                .context("Failed to get the parent directory")?;
            std::fs::create_dir_all(parent_dir).with_context(|| {
                format!(
                    "Failed to create the parent directory {}",
                    parent_dir.display()
                )
            })?;

            let write_result = std::fs::write(&file_path, content);
            match write_result {
                Ok(()) => {
                    response.push(format!("Wrote to {}", file_path.display()));
                }
                Err(e) => {
                    response.push(format!("Could not write to {}: {e}", file_path.display()));
                }
            }
        }
        Ok(response.join("\n"))
    }
}
