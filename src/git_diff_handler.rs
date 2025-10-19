use crate::constants::{MAX_DIFF_FILES, MAX_DIFF_LINES_PER_FILE};
use regex::Regex;
use std::sync::OnceLock;

static GITHUB_COMMIT_PATTERN: OnceLock<Regex> = OnceLock::new();

pub struct GitHubCommitDiff {
    pub owner: String,
    pub repo: String,
    pub commit: String,
    #[allow(dead_code)]
    pub diff_hash: Option<String>,
}

#[derive(Debug)]
pub struct FileDiff {
    pub file_path: String,
    pub changes: String,
    pub additions: usize,
    pub deletions: usize,
}

impl GitHubCommitDiff {
    pub fn parse(url: &str) -> Option<Self> {
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
            owner,
            repo,
            commit,
            diff_hash,
        })
    }

    pub fn diff_url(&self) -> String {
        format!(
            "https://github.com/{}/{}/commit/{}.diff",
            self.owner, self.repo, self.commit
        )
    }

    pub fn fetch_diff(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let response = ureq::get(&self.diff_url()).call()?;
        Ok(response.into_string()?)
    }

    pub fn parse_diff_files(&self, diff_content: &str) -> Vec<FileDiff> {
        let mut files = Vec::new();
        let mut file_sections: Vec<&str> = diff_content.split("diff --git").collect();

        file_sections.retain(|s| !s.trim().is_empty());

        for section in file_sections.iter().take(MAX_DIFF_FILES) {
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
        let mut line_count = 0;

        for line in lines.iter() {
            if line.starts_with("@@") || line.starts_with("+") || line.starts_with("-") {
                if line_count < MAX_DIFF_LINES_PER_FILE {
                    change_lines.push(*line);
                    line_count += 1;
                }

                if line.starts_with("+") && !line.starts_with("+++") {
                    additions += 1;
                } else if line.starts_with("-") && !line.starts_with("---") {
                    deletions += 1;
                }
            }
        }

        if line_count >= MAX_DIFF_LINES_PER_FILE && additions + deletions > MAX_DIFF_LINES_PER_FILE {
            change_lines.push("...");
        }

        Some(FileDiff {
            file_path,
            changes: change_lines.join("\n"),
            additions,
            deletions,
        })
    }

    pub fn format_diff_response(&self) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let diff_content = self.fetch_diff()?;
        let files = self.parse_diff_files(&diff_content);

        if files.is_empty() {
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
