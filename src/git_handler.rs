use regex::Regex;
use std::sync::OnceLock;

static GITHUB_PATTERN: OnceLock<Regex> = OnceLock::new();
static GITLAB_PATTERN: OnceLock<Regex> = OnceLock::new();
static GITEA_PATTERN: OnceLock<Regex> = OnceLock::new();
static RUSTDOC_PATTERN: OnceLock<Regex> = OnceLock::new();

const MAX_ENTIRE_FILE_LINES: usize = 20;
const SPACES_PER_TAB: &str = "    ";

#[derive(Debug, PartialEq)]
pub enum GitPlatform {
    GitHub,
    GitLab,
    Gitea,
    RustDoc,
}

pub struct GitFileLink {
    #[allow(dead_code)]
    pub platform: GitPlatform,
    #[allow(dead_code)]
    pub original_url: String,
    pub raw_url: String,
    pub file_name: String,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
}

impl GitFileLink {
    pub fn parse(url: &str) -> Option<Self> {
        Self::parse_rustdoc(url)
            .or_else(|| Self::parse_github(url))
            .or_else(|| Self::parse_gitlab(url))
            .or_else(|| Self::parse_gitea(url))
    }

    fn parse_github(url: &str) -> Option<Self> {
        let pattern = GITHUB_PATTERN.get_or_init(|| {
            Regex::new(r"https?://(?:www\.)?github\.com/([^/]+)/([^/]+)/blob/([^/]+)/([^#?]+)(?:\?[^#]*)?(?:#L(\d+)(?:-L?(\d+))?)?")
                .unwrap()
        });

        let captures = pattern.captures(url)?;
        let owner = captures.get(1)?.as_str();
        let repo = captures.get(2)?.as_str();
        let commit = captures.get(3)?.as_str();
        let path = captures.get(4)?.as_str();
        let start_line = captures.get(5).and_then(|m| m.as_str().parse().ok());
        let end_line = captures.get(6).and_then(|m| m.as_str().parse().ok());

        let raw_url = format!(
            "https://raw.githubusercontent.com/{}/{}/{}/{}",
            owner, repo, commit, path
        );
        let file_name = path.split('/').last().unwrap_or(path).to_string();

        Some(Self {
            platform: GitPlatform::GitHub,
            original_url: url.to_string(),
            raw_url,
            file_name,
            start_line,
            end_line,
        })
    }

    fn parse_gitlab(url: &str) -> Option<Self> {
        let pattern = GITLAB_PATTERN.get_or_init(|| {
            Regex::new(r"https?://(.+?)/([^/]+)/([^/]+)/-/blob/([^/]+)/([^#?]+)(?:\?[^#]*)?(?:#L(\d+)(?:-L?(\d+))?)?")
                .unwrap()
        });

        let captures = pattern.captures(url)?;
        let host = captures.get(1)?.as_str();
        let owner = captures.get(2)?.as_str();
        let repo = captures.get(3)?.as_str();
        let commit = captures.get(4)?.as_str();
        let path = captures.get(5)?.as_str();
        let start_line = captures.get(6).and_then(|m| m.as_str().parse().ok());
        let end_line = captures.get(7).and_then(|m| m.as_str().parse().ok());

        let raw_url = format!(
            "https://{}/{}/{}/-/raw/{}/{}",
            host, owner, repo, commit, path
        );
        let file_name = path.split('/').last().unwrap_or(path).to_string();

        Some(Self {
            platform: GitPlatform::GitLab,
            original_url: url.to_string(),
            raw_url,
            file_name,
            start_line,
            end_line,
        })
    }

    fn parse_rustdoc(url: &str) -> Option<Self> {
        let pattern = RUSTDOC_PATTERN.get_or_init(|| {
            Regex::new(r"https?://([^/]+)/([^/]+)/([^/]+)/src/([^/]+)/([^#]+\.rs)\.html(?:#(\d+)(?:-(\d+))?)?")
                .unwrap()
        });

        let captures = pattern.captures(url)?;
        let host = captures.get(1)?.as_str();
        let project = captures.get(2)?.as_str();
        let version = captures.get(3)?.as_str();
        let crate_name = captures.get(4)?.as_str();
        let file_path = captures.get(5)?.as_str();
        let start_line = captures.get(6).and_then(|m| m.as_str().parse().ok());
        let end_line = captures.get(7).and_then(|m| m.as_str().parse().ok());

        let raw_url = format!("https://{}/{}/{}/src/{}/{}", host, project, version, crate_name, file_path);
        let file_name = file_path.split('/').last().unwrap_or(file_path);

        Some(Self {
            platform: GitPlatform::RustDoc,
            original_url: url.to_string(),
            raw_url,
            file_name: file_name.to_string(),
            start_line,
            end_line,
        })
    }

    fn parse_gitea(url: &str) -> Option<Self> {
        let pattern = GITEA_PATTERN.get_or_init(|| {
            Regex::new(r"https?://(.+?)/([^/]+)/([^/]+)/src/(?:branch|commit)/([^/]+)/([^#?]+)(?:\?[^#]*)?(?:#L(\d+)(?:-L?(\d+))?)?")
                .unwrap()
        });

        let captures = pattern.captures(url)?;
        let host = captures.get(1)?.as_str();
        let owner = captures.get(2)?.as_str();
        let repo = captures.get(3)?.as_str();
        let commit = captures.get(4)?.as_str();
        let path = captures.get(5)?.as_str();
        let start_line = captures.get(6).and_then(|m| m.as_str().parse().ok());
        let end_line = captures.get(7).and_then(|m| m.as_str().parse().ok());

        let raw_url = format!(
            "https://{}/{}/{}/raw/{}/{}",
            host, owner, repo, commit, path
        );
        let file_name = path.split('/').last().unwrap_or(path).to_string();

        Some(Self {
            platform: GitPlatform::Gitea,
            original_url: url.to_string(),
            raw_url,
            file_name,
            start_line,
            end_line,
        })
    }

    pub fn fetch_content(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let response = ureq::get(&self.raw_url).call()?;
        let content = response.into_string()?;

        if self.platform == GitPlatform::RustDoc {
            Ok(Self::strip_html(&content))
        } else {
            Ok(content)
        }
    }

    fn strip_html(html: &str) -> String {
        let lines: Vec<String> = html.lines().map(|line| {
            let mut result = String::new();
            let mut in_tag = false;
            let mut chars = line.chars().peekable();

            while let Some(ch) = chars.next() {
                if ch == '<' {
                    in_tag = true;
                } else if ch == '>' {
                    in_tag = false;
                } else if !in_tag {
                    if ch == '&' {
                        let entity: String = chars.by_ref().take_while(|&c| c != ';').collect();
                        match entity.as_str() {
                            "lt" => result.push('<'),
                            "gt" => result.push('>'),
                            "amp" => result.push('&'),
                            "quot" => result.push('"'),
                            "apos" | "#39" => result.push('\''),
                            _ => {
                                result.push('&');
                                result.push_str(&entity);
                                result.push(';');
                            }
                        }
                    } else {
                        result.push(ch);
                    }
                }
            }
            result
        }).collect();

        lines.join("\n")
    }

    fn get_extension(&self) -> &str {
        self.file_name.rsplit('.').next().unwrap_or("")
    }

    pub fn get_language(&self) -> &str {
        match self.get_extension() {
            "astro" | "svelte" | "vue" => "jsx",
            "mdx" => "md",
            "jsonc" | "json5" | "jsonld" => "json",
            "sublime-build" | "sublime-settings" | "sublime-menu" | "sublime-commands"
            | "sublime-project" | "sublime-mousemap" | "sublime-keymap" | "sublime-macro"
            | "sublime-completions" | "code-workspace" | "code-snippets" => "json",
            ext => ext,
        }
    }

    fn unindent(text: &str) -> String {
        let text = text.replace('\t', SPACES_PER_TAB);
        let lines: Vec<&str> = text.lines().collect();

        if lines.is_empty() {
            return text;
        }

        let min_indent = lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| Some(line.len() - line.trim_start().len()))
            .min()
            .unwrap_or(0);

        if min_indent == 0 {
            return text;
        }

        lines
            .iter()
            .map(|line| {
                if line.len() >= min_indent {
                    &line[min_indent..]
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn extract_lines(&self, content: &str) -> Option<String> {
        let lines: Vec<&str> = content.lines().collect();

        let extracted = if let (Some(start), Some(end)) = (self.start_line, self.end_line) {
            let start_idx = (start as usize).saturating_sub(1);
            let end_idx = (end as usize).min(lines.len());

            if start_idx >= lines.len() {
                return None;
            }

            lines[start_idx..end_idx].join("\n")
        } else if let Some(line) = self.start_line {
            let idx = (line as usize).saturating_sub(1);
            lines.get(idx)?.to_string()
        } else {
            if self.get_language() == "md" {
                return None;
            }

            if lines.len() > MAX_ENTIRE_FILE_LINES {
                return None;
            }

            content.to_string()
        };

        Some(Self::unindent(&extracted))
    }

    pub fn format_response(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let content = self.fetch_content()?;
        let extracted = self.extract_lines(&content).ok_or("File too long or invalid")?;
        let language = self.get_language();

        let line_info = match (self.start_line, self.end_line) {
            (Some(start), Some(end)) if start == end => format!(" Line {}", start),
            (Some(start), Some(end)) => format!(" Lines {}-{}", start, end),
            (Some(line), None) => format!(" Line {}", line),
            _ => String::new(),
        };

        Ok(format!(
            "**{}:**{}\n```{}\n{}\n```",
            self.file_name, line_info, language, extracted
        ))
    }
}
