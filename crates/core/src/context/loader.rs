//! Context Loader
//!
//! Loads and parses project context files (AETHER.md, MEMORY.md, etc.)
//! that provide AI assistants with project-specific knowledge.

use std::path::{Path, PathBuf};
use std::time::Duration;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// Maximum total memory size (1MB)
/// Note: Reserved for future memory limiting feature.
#[allow(dead_code)]
const MAX_TOTAL_SIZE: usize = 1024 * 1024;

/// Maximum entries
/// Note: Reserved for future entry limiting feature.
#[allow(dead_code)]
const MAX_ENTRIES: usize = 1000;

/// Maximum entry age (7 days)
/// Note: Reserved for future entry expiration feature.
#[allow(dead_code)]
const MAX_ENTRY_AGE: std::time::Duration = Duration::from_secs(60 * 60 * 24 * 7);

/// Type of context file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContextFileType {
    /// Main project context (AETHER.md)
    Project,
    /// Memory/persistent context (MEMORY.md)
    Memory,
    /// Session-specific context
    Session,
    /// Custom context file
    Custom,
}

impl Default for ContextFileType {
    fn default() -> Self {
        Self::Project
    }
}

/// A section within a context file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSection {
    /// Section heading (without the ## prefix)
    pub heading: String,
    /// Section level (1-6 for # to ######)
    pub level: u8,
    /// Section content
    pub content: String,
    /// Line number where section starts (1-indexed)
    pub line_number: usize,
    /// Tags extracted from content (e.g., #tag, @tag)
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Default for ContextSection {
    fn default() -> Self {
        Self {
            heading: String::new(),
            level: 2,
            content: String::new(),
            line_number: 1,
            tags: Vec::new(),
        }
    }
}

/// A parsed context file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFile {
    /// File path
    pub path: PathBuf,
    /// Type of context file
    pub file_type: ContextFileType,
    /// Raw file content
    pub raw_content: String,
    /// Parsed sections
    pub sections: Vec<ContextSection>,
    /// File title (first H1 heading)
    pub title: Option<String>,
    /// Last modified timestamp (Unix epoch)
    pub modified: Option<u64>,
}

impl Default for ContextFile {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            file_type: ContextFileType::Project,
            raw_content: String::new(),
            sections: Vec::new(),
            title: None,
            modified: None,
        }
    }
}

/// Configuration for context loading
#[derive(Debug, Clone)]
pub struct ContextConfig {
    /// Root directory to search for context files
    pub root_dir: PathBuf,
    /// Names of context files to look for (in priority order)
    pub context_files: Vec<String>,
    /// Maximum file size to load (in bytes)
    pub max_file_size: usize,
    /// Whether to follow symlinks
    pub follow_symlinks: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            root_dir: PathBuf::from("."),
            context_files: vec![
                "AETHER.md".to_string(),
                "MEMORY.md".to_string(),
                "CONTEXT.md".to_string(),
            ],
            max_file_size: 1024 * 1024, // 1MB
            follow_symlinks: false,
        }
    }
}

impl ContextConfig {
    /// Create a new config with the given root directory
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: root_dir.into(),
            ..Default::default()
        }
    }
}

/// Context loader for parsing and managing context files
#[derive(Debug)]
pub struct ContextLoader {
    config: ContextConfig,
    loaded_files: RwLock<Vec<ContextFile>>,
}

impl Default for ContextLoader {
    fn default() -> Self {
        Self::new(ContextConfig::default())
    }
}

impl ContextLoader {
    /// Create a new context loader with the given config
    pub fn new(config: ContextConfig) -> Self {
        Self {
            config,
            loaded_files: RwLock::new(Vec::new()),
        }
    }

    /// Create a context loader for a given directory
    pub fn for_dir(dir: impl Into<PathBuf>) -> Self {
        Self::new(ContextConfig::new(dir))
    }

    /// Load all context files from the configured directory
    pub fn load_all(&self) -> Result<Vec<ContextFile>, ContextLoadError> {
        let mut files = Vec::new();

        for filename in &self.config.context_files {
            let path = self.config.root_dir.join(filename);
            if path.exists() {
                match self.load_file(&path) {
                    Ok(file) => files.push(file),
                    Err(e) => {
                        // Log but continue with other files
                        tracing::warn!("Failed to load context file {:?}: {}", path, e);
                    }
                }
            }
        }

        // Update cache
        *self.loaded_files.write() = files.clone();

        Ok(files)
    }

    /// Load a specific context file
    pub fn load_file(&self, path: &Path) -> Result<ContextFile, ContextLoadError> {
        // Check file exists
        if !path.exists() {
            return Err(ContextLoadError::FileNotFound(path.to_path_buf()));
        }

        // Check file size
        let metadata =
            std::fs::metadata(path).map_err(|e| ContextLoadError::Io(path.to_path_buf(), e))?;

        if metadata.len() as usize > self.config.max_file_size {
            return Err(ContextLoadError::FileTooLarge(
                path.to_path_buf(),
                metadata.len() as usize,
                self.config.max_file_size,
            ));
        }

        // Read content
        let raw_content = std::fs::read_to_string(path)
            .map_err(|e| ContextLoadError::Io(path.to_path_buf(), e))?;

        // Determine file type
        let file_type = Self::detect_file_type(path);

        // Parse sections
        let (title, sections) = Self::parse_content(&raw_content);

        // Get modified time
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());

        Ok(ContextFile {
            path: path.to_path_buf(),
            file_type,
            raw_content,
            sections,
            title,
            modified,
        })
    }

    /// Detect the type of context file from its name
    fn detect_file_type(path: &Path) -> ContextFileType {
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        match filename.to_uppercase().as_str() {
            "AETHER.MD" | "AETHER" => ContextFileType::Project,
            "MEMORY.MD" | "MEMORY" => ContextFileType::Memory,
            "SESSION.MD" | "SESSION" => ContextFileType::Session,
            _ => ContextFileType::Custom,
        }
    }

    /// Parse content into title and sections
    fn parse_content(content: &str) -> (Option<String>, Vec<ContextSection>) {
        let mut title = None;
        let mut sections = Vec::new();
        let mut current_section: Option<ContextSection> = None;
        let mut line_number = 0;

        for line in content.lines() {
            line_number += 1;

            // Check for H1 heading (title)
            if line.starts_with("# ") && !line.starts_with("## ") {
                title = Some(line[2..].trim().to_string());
                continue;
            }

            // Check for section headings (H2-H6)
            let heading_level = Self::get_heading_level(line);
            if heading_level >= 2 {
                // Save previous section
                if let Some(section) = current_section.take() {
                    sections.push(section);
                }

                // Start new section
                let heading = line[heading_level as usize..].trim().to_string();
                current_section = Some(ContextSection {
                    heading,
                    level: heading_level,
                    content: String::new(),
                    line_number,
                    tags: Vec::new(),
                });
            } else if let Some(ref mut section) = current_section {
                // Add content to current section
                if !section.content.is_empty() {
                    section.content.push('\n');
                }
                section.content.push_str(line);

                // Extract tags (words starting with # or @)
                for word in line.split_whitespace() {
                    if word.starts_with('#') || word.starts_with('@') {
                        let tag = word
                            .trim_matches(|c| c == '#' || c == '@' || c == ':' || c == ',')
                            .to_string();
                        if !tag.is_empty() && !section.tags.contains(&tag) {
                            section.tags.push(tag);
                        }
                    }
                }
            }
        }

        // Save last section
        if let Some(section) = current_section {
            sections.push(section);
        }

        (title, sections)
    }

    /// Get the heading level from a line (1-6, or 0 if not a heading)
    fn get_heading_level(line: &str) -> u8 {
        let mut count = 0u8;
        for c in line.chars() {
            if c == '#' {
                count += 1;
            } else {
                break;
            }
        }
        if count >= 1 && count <= 6 && line.chars().nth(count as usize) == Some(' ') {
            count
        } else {
            0
        }
    }

    /// Get a section by heading name
    pub fn get_section(&self, heading: &str) -> Option<ContextSection> {
        let files = self.loaded_files.read();
        for file in files.iter() {
            for section in &file.sections {
                if section.heading.eq_ignore_ascii_case(heading) {
                    return Some(section.clone());
                }
            }
        }
        None
    }

    /// Get all loaded sections across all files
    pub fn get_all_sections(&self) -> Vec<ContextSection> {
        let files = self.loaded_files.read();
        files.iter().flat_map(|f| f.sections.clone()).collect()
    }

    /// Get all loaded files
    pub fn get_files(&self) -> Vec<ContextFile> {
        self.loaded_files.read().clone()
    }

    /// Reload all context files
    pub fn reload(&self) -> Result<Vec<ContextFile>, ContextLoadError> {
        self.load_all()
    }

    /// Check if any context files are loaded
    pub fn is_loaded(&self) -> bool {
        !self.loaded_files.read().is_empty()
    }

    /// Get the combined content of all loaded files
    pub fn combined_content(&self) -> String {
        let files = self.loaded_files.read();
        files
            .iter()
            .map(|f| f.raw_content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    }

    /// Get the root directory
    pub fn root_dir(&self) -> &Path {
        &self.config.root_dir
    }
}

/// Errors that can occur during context loading
#[derive(Debug, thiserror::Error)]
pub enum ContextLoadError {
    /// File not found
    #[error("Context file not found: {0}")]
    FileNotFound(PathBuf),

    /// File too large
    #[error("Context file too large: {0} ({1} bytes, max {2})")]
    FileTooLarge(PathBuf, usize, usize),

    /// IO error
    #[error("IO error reading {0}: {1}")]
    Io(PathBuf, #[source] std::io::Error),

    /// Parse error
    #[error("Parse error in {0}: {1}")]
    Parse(PathBuf, String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_simple_content() {
        let content = r#"# Project Title

## Overview
This is the project overview.

## Goals
- Goal 1
- Goal 2

## Architecture
The system architecture is...
"#;

        let (title, sections) = ContextLoader::parse_content(content);

        assert_eq!(title, Some("Project Title".to_string()));
        assert_eq!(sections.len(), 3);

        assert_eq!(sections[0].heading, "Overview");
        assert_eq!(sections[0].level, 2);

        assert_eq!(sections[1].heading, "Goals");
        assert!(sections[1].content.contains("Goal 1"));

        assert_eq!(sections[2].heading, "Architecture");
    }

    #[test]
    fn test_parse_nested_sections() {
        let content = r#"## Main Section
Content for main section.

### Subsection
Content for subsection.

#### Deep subsection
Deep content.

## Another Main
More content.
"#;

        let (_, sections) = ContextLoader::parse_content(content);

        assert_eq!(sections.len(), 4);
        assert_eq!(sections[0].heading, "Main Section");
        assert_eq!(sections[0].level, 2);
        assert_eq!(sections[1].heading, "Subsection");
        assert_eq!(sections[1].level, 3);
        assert_eq!(sections[2].heading, "Deep subsection");
        assert_eq!(sections[2].level, 4);
        assert_eq!(sections[3].heading, "Another Main");
        assert_eq!(sections[3].level, 2);
    }

    #[test]
    fn test_get_heading_level() {
        assert_eq!(ContextLoader::get_heading_level("# Title"), 1);
        assert_eq!(ContextLoader::get_heading_level("## Section"), 2);
        assert_eq!(ContextLoader::get_heading_level("### Subsection"), 3);
        assert_eq!(ContextLoader::get_heading_level("#### Deep"), 4);
        assert_eq!(ContextLoader::get_heading_level("##### Deeper"), 5);
        assert_eq!(ContextLoader::get_heading_level("###### Deepest"), 6);
        assert_eq!(ContextLoader::get_heading_level("Not a heading"), 0);
        assert_eq!(ContextLoader::get_heading_level("####### Too many"), 0);
        assert_eq!(ContextLoader::get_heading_level("#No space"), 0);
    }

    #[test]
    fn test_detect_file_type() {
        assert_eq!(
            ContextLoader::detect_file_type(Path::new("AETHER.md")),
            ContextFileType::Project
        );
        assert_eq!(
            ContextLoader::detect_file_type(Path::new("MEMORY.md")),
            ContextFileType::Memory
        );
        assert_eq!(
            ContextLoader::detect_file_type(Path::new("SESSION.md")),
            ContextFileType::Session
        );
        assert_eq!(
            ContextLoader::detect_file_type(Path::new("CUSTOM.md")),
            ContextFileType::Custom
        );
    }

    #[test]
    fn test_load_file() {
        let temp_dir = TempDir::new().unwrap();
        let context_path = temp_dir.path().join("AETHER.md");

        let content = r#"# Test Project

## Overview
Test overview.
"#;
        fs::write(&context_path, content).unwrap();

        let loader = ContextLoader::for_dir(temp_dir.path());
        let file = loader.load_file(&context_path).unwrap();

        assert_eq!(file.title, Some("Test Project".to_string()));
        assert_eq!(file.file_type, ContextFileType::Project);
        assert_eq!(file.sections.len(), 1);
        assert_eq!(file.sections[0].heading, "Overview");
    }

    #[test]
    fn test_load_all() {
        let temp_dir = TempDir::new().unwrap();

        // Create AETHER.md
        fs::write(
            temp_dir.path().join("AETHER.md"),
            "# Project\n\n## Section\nContent\n",
        )
        .unwrap();

        // Create MEMORY.md
        fs::write(
            temp_dir.path().join("MEMORY.md"),
            "# Memory\n\n## Notes\nNote content\n",
        )
        .unwrap();

        let loader = ContextLoader::for_dir(temp_dir.path());
        let files = loader.load_all().unwrap();

        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_get_section() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(
            temp_dir.path().join("AETHER.md"),
            "# Project\n\n## Goals\n- Goal 1\n\n## Architecture\nArch content\n",
        )
        .unwrap();

        let loader = ContextLoader::for_dir(temp_dir.path());
        loader.load_all().unwrap();

        let section = loader.get_section("Goals").unwrap();
        assert_eq!(section.heading, "Goals");

        let section = loader.get_section("GOALS").unwrap(); // Case insensitive
        assert_eq!(section.heading, "Goals");

        assert!(loader.get_section("Nonexistent").is_none());
    }

    #[test]
    fn test_combined_content() {
        let temp_dir = TempDir::new().unwrap();

        fs::write(temp_dir.path().join("AETHER.md"), "Project content").unwrap();
        fs::write(temp_dir.path().join("MEMORY.md"), "Memory content").unwrap();

        let loader = ContextLoader::for_dir(temp_dir.path());
        loader.load_all().unwrap();

        let combined = loader.combined_content();
        assert!(combined.contains("Project content"));
        assert!(combined.contains("Memory content"));
    }

    #[test]
    fn test_file_not_found() {
        let loader = ContextLoader::for_dir("/nonexistent");
        let result = loader.load_file(Path::new("/nonexistent/AETHER.md"));
        assert!(matches!(result, Err(ContextLoadError::FileNotFound(_))));
    }

    #[test]
    fn test_file_too_large() {
        let temp_dir = TempDir::new().unwrap();
        let large_path = temp_dir.path().join("AETHER.md");

        // Write a file larger than 100 bytes
        let mut file = fs::File::create(&large_path).unwrap();
        write!(file, "{}", "x".repeat(200)).unwrap();

        let mut config = ContextConfig::new(temp_dir.path());
        config.max_file_size = 100;
        let loader = ContextLoader::new(config);

        let result = loader.load_file(&large_path);
        assert!(matches!(
            result,
            Err(ContextLoadError::FileTooLarge(_, _, _))
        ));
    }
}
