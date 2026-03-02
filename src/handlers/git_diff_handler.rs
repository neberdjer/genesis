use crate::constants::MAX_DIFF_CHARS_PER_FILE;
use regex::Regex;
use serde_json;
use std::sync::OnceLock;

static GITHUB_COMMIT_PATTERN: OnceLock<Regex> = OnceLock::new();
static GITHUB_COMPARE_PATTERN: OnceLock<Regex> = OnceLock::new();
static GITLAB_COMMIT_PATTERN: OnceLock<Regex> = OnceLock::new();
static GITLAB_COMPARE_PATTERN: OnceLock<Regex> = OnceLock::new();

#[derive(Debug, PartialEq)]
pub enum GitPlatform {
    GitHub,
    GitLab,
}

pub struct CommitDiff {
    pub platform: GitPlatform,
    pub owner: String,
    pub repo: String,
    pub commit: String,
    #[allow(dead_code)]
    pub diff_hash: Option<String>,
    pub host: Option<String>,
    pub file_filter: Option<String>,
    pub is_compare: bool,
}

#[derive(Debug)]
pub struct FileDiff {
    pub file_path: String,
    pub changes: String,
    pub additions: usize,
    pub deletions: usize,
}

impl CommitDiff {
    pub fn parse(url: &str) -> Option<Self> {
        Self::parse_github(url)
            .or_else(|| Self::parse_github_compare(url))
            .or_else(|| Self::parse_gitlab(url))
            .or_else(|| Self::parse_gitlab_compare(url))
    }

    fn parse_github(url: &str) -> Option<Self> {
        let pattern = GITHUB_COMMIT_PATTERN.get_or_init(|| {
            Regex::new(r"https?://(?:www\.)?github\.com/([^/]+)/([^/]+)/commit/([a-f0-9]+)(?:#diff-([a-f0-9]+))?")
                .unwrap()
        });

        let captures = pattern.captures(url)?;
        let owner = captures.get(1)?.as_str().to_string();
        let repo = captures.get(2)?.as_str().to_string();
        let commit = captures.get(3)?.as_str().to_string();
        let diff_hash = captures.get(4).map(|m| m.as_str().to_string());

        Some(Self {
            platform: GitPlatform::GitHub,
            owner,
            repo,
            commit,
            diff_hash,
            host: None,
            file_filter: None,
            is_compare: false,
        })
    }

    fn parse_github_compare(url: &str) -> Option<Self> {
        let pattern = GITHUB_COMPARE_PATTERN.get_or_init(|| {
            Regex::new(r"https?://(?:www\.)?github\.com/([^/]+)/([^/]+)/compare/(.+)").unwrap()
        });

        let captures = pattern.captures(url)?;
        let owner = captures.get(1)?.as_str().to_string();
        let repo = captures.get(2)?.as_str().to_string();
        let compare_range = captures.get(3)?.as_str().to_string();

        Some(Self {
            platform: GitPlatform::GitHub,
            owner,
            repo,
            commit: compare_range,
            diff_hash: None,
            host: None,
            file_filter: None,
            is_compare: true,
        })
    }

    fn parse_gitlab(url: &str) -> Option<Self> {
        let pattern = GITLAB_COMMIT_PATTERN.get_or_init(|| {
            Regex::new(r"https?://([^/]+)/(.+?)/-/commit/([a-f0-9]+)(?:#diff-([a-f0-9]+))?")
                .unwrap()
        });

        let captures = pattern.captures(url)?;
        let host = captures.get(1)?.as_str().to_string();
        let full_path = captures.get(2)?.as_str();
        let commit = captures.get(3)?.as_str().to_string();
        let diff_hash = captures.get(4).map(|m| m.as_str().to_string());

        let path_parts: Vec<&str> = full_path.split('/').collect();
        if path_parts.is_empty() {
            return None;
        }

        let repo = path_parts.last()?.to_string();
        let owner = path_parts[..path_parts.len() - 1].join("/");

        Some(Self {
            platform: GitPlatform::GitLab,
            owner,
            repo,
            commit,
            diff_hash,
            host: Some(host),
            file_filter: None,
            is_compare: false,
        })
    }

    fn parse_gitlab_compare(url: &str) -> Option<Self> {
        let pattern = GITLAB_COMPARE_PATTERN
            .get_or_init(|| Regex::new(r"https?://([^/]+)/(.+?)/-/compare/([^?]+)").unwrap());

        let captures = pattern.captures(url)?;
        let host = captures.get(1)?.as_str().to_string();
        let full_path = captures.get(2)?.as_str();
        let compare_range = captures.get(3)?.as_str().to_string();

        let path_parts: Vec<&str> = full_path.split('/').collect();
        if path_parts.is_empty() {
            return None;
        }

        let repo = path_parts.last()?.to_string();
        let owner = path_parts[..path_parts.len() - 1].join("/");

        Some(Self {
            platform: GitPlatform::GitLab,
            owner,
            repo,
            commit: compare_range,
            diff_hash: None,
            host: Some(host),
            file_filter: None,
            is_compare: true,
        })
    }

    pub fn diff_url(&self) -> String {
        match self.platform {
            GitPlatform::GitHub => {
                if self.is_compare {
                    format!(
                        "https://github.com/{}/{}/compare/{}.diff",
                        self.owner, self.repo, self.commit
                    )
                } else {
                    format!(
                        "https://github.com/{}/{}/commit/{}.diff",
                        self.owner, self.repo, self.commit
                    )
                }
            }
            GitPlatform::GitLab => {
                let host = self.host.as_deref().unwrap_or("gitlab.com");
                let full_path = if self.owner.is_empty() {
                    self.repo.clone()
                } else {
                    format!("{}/{}", self.owner, self.repo)
                };
                if self.is_compare {
                    let parts: Vec<&str> = self.commit.split("...").collect();
                    if parts.len() == 2 {
                        let from_branch = parts[0];
                        let to_branch = parts[1];
                        let project_path = urlencoding::encode(&full_path);
                        format!(
                            "https://{}/api/v4/projects/{}/repository/compare?from={}&to={}&straight=true",
                            host, project_path, from_branch, to_branch
                        )
                    } else {
                        format!(
                            "https://{}/{}/-/compare/{}.diff",
                            host, full_path, self.commit
                        )
                    }
                } else {
                    format!(
                        "https://{}/{}/-/commit/{}.diff",
                        host, full_path, self.commit
                    )
                }
            }
        }
    }

    pub fn fetch_diff(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let url = self.diff_url();
        tracing::debug!("Fetching diff from: {}", url);
        let response = ureq::get(&url).call()?;

        if self.platform == GitPlatform::GitLab && self.is_compare {
            let json: serde_json::Value = response.into_json()?;

            if let Some(diffs_array) = json.get("diffs").and_then(|v| v.as_array()) {
                let mut full_diff = String::new();

                for diff_obj in diffs_array {
                    let old_path = diff_obj
                        .get("old_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let new_path = diff_obj
                        .get("new_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let diff_content = diff_obj.get("diff").and_then(|v| v.as_str()).unwrap_or("");

                    full_diff.push_str(&format!("diff --git a/{} b/{}\n", old_path, new_path));
                    full_diff.push_str(&format!("--- a/{}\n", old_path));
                    full_diff.push_str(&format!("+++ b/{}\n", new_path));
                    full_diff.push_str(diff_content);
                    full_diff.push('\n');
                }

                tracing::debug!(
                    "Reconstructed diff from GitLab API, length: {}",
                    full_diff.len()
                );
                return Ok(full_diff);
            } else {
                return Err("No diffs found in GitLab API response".into());
            }
        }

        let content = response.into_string()?;
        tracing::debug!("Diff content length: {}", content.len());
        Ok(content)
    }

    pub fn parse_diff_files(&self, diff_content: &str) -> Vec<FileDiff> {
        let mut files = Vec::new();
        let mut file_sections: Vec<&str> = diff_content.split("diff --git").collect();

        file_sections.retain(|s| !s.trim().is_empty());
        tracing::debug!("Found {} file sections in diff", file_sections.len());

        for (i, section) in file_sections.iter().enumerate() {
            if let Some(file_diff) = Self::parse_file_diff(section) {
                tracing::debug!(
                    "Parsed file {}: {} (+{} -{})",
                    i,
                    file_diff.file_path,
                    file_diff.additions,
                    file_diff.deletions
                );
                files.push(file_diff);
            } else {
                tracing::debug!("Failed to parse file section {}", i);
            }
        }

        tracing::debug!("Total files parsed: {}", files.len());
        files
    }

    fn parse_file_diff(section: &str) -> Option<FileDiff> {
        let lines: Vec<&str> = section.lines().collect();
        if lines.is_empty() {
            tracing::debug!("Empty section");
            return None;
        }

        tracing::debug!(
            "First 3 lines of section: {:?}",
            &lines[..lines.len().min(3)]
        );

        let file_path = lines
            .iter()
            .find(|line| line.starts_with("+++"))
            .and_then(|line| line.strip_prefix("+++ b/"))
            .unwrap_or("unknown")
            .to_string();

        let mut additions = 0;
        let mut deletions = 0;
        let mut change_lines = Vec::new();
        let mut char_count = 0;
        let mut truncated = false;

        for line in lines.iter() {
            if line.starts_with("@@") || line.starts_with("+") || line.starts_with("-") {
                let line_with_newline = line.len() + 1;
                if char_count + line_with_newline <= MAX_DIFF_CHARS_PER_FILE {
                    change_lines.push(*line);
                    char_count += line_with_newline;
                } else if !truncated {
                    change_lines.push("...");
                    truncated = true;
                }

                if line.starts_with("+") && !line.starts_with("+++") {
                    additions += 1;
                } else if line.starts_with("-") && !line.starts_with("---") {
                    deletions += 1;
                }
            }
        }

        Some(FileDiff {
            file_path,
            changes: change_lines.join("\n"),
            additions,
            deletions,
        })
    }

    fn matches_filter(&self, file_path: &str) -> bool {
        match &self.file_filter {
            None => true,
            Some(filter) => {
                file_path.contains(filter)
                    || file_path.ends_with(filter)
                    || file_path
                        .split('/')
                        .last()
                        .map(|name| name == filter)
                        .unwrap_or(false)
            }
        }
    }

    pub fn format_diff_response(
        &self,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let diff_content = self.fetch_diff()?;
        let mut files = self.parse_diff_files(&diff_content);

        if self.file_filter.is_some() {
            files.retain(|file| self.matches_filter(&file.file_path));
        }

        if files.is_empty() {
            if self.file_filter.is_some() {
                return Err(format!(
                    "No file matching '{}' found in this commit",
                    self.file_filter.as_ref().unwrap()
                )
                .into());
            }
            return Err("No files changed in this commit".into());
        }

        let mut responses = Vec::new();

        for file in files {
            let stats = format!("+{} -{}", file.additions, file.deletions);
            let file_name = file.file_path.split('/').last().unwrap_or(&file.file_path);

            let response = format!(
                "**{}** `{}`\n```diff\n{}\n```",
                file_name, stats, file.changes.replace("```", "`\\``")
            );

            responses.push(response);
        }

        Ok(responses)
    }
}
