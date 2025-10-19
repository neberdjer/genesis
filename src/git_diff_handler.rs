use crate::constants::MAX_DIFF_CHARS_PER_FILE;
use regex::Regex;
use std::sync::OnceLock;

static GITHUB_COMMIT_PATTERN: OnceLock<Regex> = OnceLock::new();
static GITLAB_COMMIT_PATTERN: OnceLock<Regex> = OnceLock::new();

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
            .or_else(|| Self::parse_gitlab(url))
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
        })
    }

    pub fn diff_url(&self) -> String {
        match self.platform {
            GitPlatform::GitHub => format!(
                "https://github.com/{}/{}/commit/{}.diff",
                self.owner, self.repo, self.commit
            ),
            GitPlatform::GitLab => {
                let host = self.host.as_deref().unwrap_or("gitlab.com");
                let full_path = if self.owner.is_empty() {
                    self.repo.clone()
                } else {
                    format!("{}/{}", self.owner, self.repo)
                };
                format!(
                    "https://{}/{}/-/commit/{}.diff",
                    host, full_path, self.commit
                )
            }
        }
    }

    pub fn fetch_diff(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let response = ureq::get(&self.diff_url()).call()?;
        Ok(response.into_string()?)
    }

    pub fn parse_diff_files(&self, diff_content: &str) -> Vec<FileDiff> {
        let mut files = Vec::new();
        let mut file_sections: Vec<&str> = diff_content.split("diff --git").collect();

        file_sections.retain(|s| !s.trim().is_empty());

        for section in file_sections.iter() {
            if let Some(file_diff) = Self::parse_file_diff(section) {
                files.push(file_diff);
            }
        }

        files
    }

    fn parse_file_diff(section: &str) -> Option<FileDiff> {
        let lines: Vec<&str> = section.lines().collect();
        if lines.is_empty() {
            return None;
        }

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
                file_path.contains(filter) ||
                file_path.ends_with(filter) ||
                file_path.split('/').last().map(|name| name == filter).unwrap_or(false)
            }
        }
    }

    pub fn format_diff_response(&self) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let diff_content = self.fetch_diff()?;
        let mut files = self.parse_diff_files(&diff_content);

        if self.file_filter.is_some() {
            files.retain(|file| self.matches_filter(&file.file_path));
        }

        if files.is_empty() {
            if self.file_filter.is_some() {
                return Err(format!("No file matching '{}' found in this commit", self.file_filter.as_ref().unwrap()).into());
            }
            return Err("No files changed in this commit".into());
        }

        let mut responses = Vec::new();

        for file in files {
            let stats = format!("+{} -{}", file.additions, file.deletions);
            let file_name = file
                .file_path
                .split('/')
                .last()
                .unwrap_or(&file.file_path);

            let response = format!(
                "**{}** `{}`\n```diff\n{}\n```",
                file_name, stats, file.changes
            );

            responses.push(response);
        }

        Ok(responses)
    }
}
