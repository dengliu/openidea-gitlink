//! GitLink for OpenIDEA: opens or copies a remote link (or Markdown link) to a
//! file, line selection, directory or commit on GitHub, GitLab, Bitbucket
//! (Cloud & Server), Codeberg, Forgejo, Gitea, Gitee, Gogs, Azure DevOps,
//! sourcehut, Gerrit, or a custom URL template.
//!
//! Ported from <https://github.com/ben-gibson/GitLink>.

use openidea_plugin::{
    Error, Invocation, NotificationLevel, Plugin, Project, Range, RepositoryStatus, host,
    register_plugin,
};

#[derive(Default)]
pub struct GitLink;

/// Supported Git hosting platforms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    GitHub,
    GitLab,
    BitbucketCloud,
    BitbucketServer,
    Codeberg,
    Forgejo,
    Gitea,
    Gitee,
    Gogs,
    Azure,
    SourceHut,
    Gerrit,
    Custom,
}

impl Platform {
    pub fn name(self) -> &'static str {
        match self {
            Self::GitHub => "GitHub",
            Self::GitLab => "GitLab",
            Self::BitbucketCloud => "Bitbucket",
            Self::BitbucketServer => "Bitbucket Server",
            Self::Codeberg => "Codeberg",
            Self::Forgejo => "Forgejo",
            Self::Gitea => "Gitea",
            Self::Gitee => "Gitee",
            Self::Gogs => "Gogs",
            Self::Azure => "Azure DevOps",
            Self::SourceHut => "sourcehut",
            Self::Gerrit => "Gerrit",
            Self::Custom => "Custom",
        }
    }

    pub fn from_setting(s: &str, host: &str) -> Self {
        match s.trim() {
            "github" => Self::GitHub,
            "gitlab" => Self::GitLab,
            "bitbucket-cloud" => Self::BitbucketCloud,
            "bitbucket-server" => Self::BitbucketServer,
            "codeberg" => Self::Codeberg,
            "forgejo" => Self::Forgejo,
            "gitea" => Self::Gitea,
            "gitee" => Self::Gitee,
            "gogs" => Self::Gogs,
            "azure" => Self::Azure,
            "srht" => Self::SourceHut,
            "gerrit" => Self::Gerrit,
            "custom" => Self::Custom,
            _ => Self::detect(host),
        }
    }

    /// Detects the hosting platform from the remote's hostname.
    pub fn detect(host: &str) -> Self {
        let h = host.trim().trim_end_matches('.').to_ascii_lowercase();
        match h.as_str() {
            "github.com" => return Self::GitHub,
            "gitlab.com" => return Self::GitLab,
            "bitbucket.org" => return Self::BitbucketCloud,
            "codeberg.org" => return Self::Codeberg,
            "gitee.com" => return Self::Gitee,
            "git.sr.ht" => return Self::SourceHut,
            "dev.azure.com" | "ssh.dev.azure.com" => return Self::Azure,
            _ => {}
        }
        if h.ends_with(".visualstudio.com") || h.contains("azure") {
            Self::Azure
        } else if h.contains("gitlab") {
            Self::GitLab
        } else if h.contains("bitbucket") || h.contains("stash") {
            Self::BitbucketServer
        } else if h.contains("forgejo") {
            Self::Forgejo
        } else if h.contains("gitea") {
            Self::Gitea
        } else if h.contains("gogs") {
            Self::Gogs
        } else if h.contains("gerrit") || h.contains("googlesource.com") {
            Self::Gerrit
        } else if h.contains("gitee") {
            Self::Gitee
        } else if h.contains("sr.ht") {
            Self::SourceHut
        } else if h.contains("codeberg") {
            Self::Codeberg
        } else {
            Self::GitHub
        }
    }

    /// Gerrit changes are pushed to `refs/for/<branch>`, so a local commit is
    /// not on the remote tracking branch; always link to the commit hash.
    pub fn commits_reachable_from_remote(self) -> bool {
        !matches!(self, Self::Gerrit)
    }

    pub fn templates(self, settings: &Settings) -> Option<UrlTemplates> {
        Some(match self {
            Self::GitHub | Self::Gitee => UrlTemplates {
                file_at_branch: "{remote:url}/{object}/{branch}/{file:path}/{file:name}{line-block:start}#L{line:start}-L{line:end}{line-block:end}".into(),
                file_at_commit: "{remote:url}/{object}/{commit}/{file:path}/{file:name}{line-block:start}#L{line:start}-L{line:end}{line-block:end}".into(),
                commit: "{remote:url}/commit/{commit}".into(),
            },
            Self::GitLab => UrlTemplates {
                file_at_branch: "{remote:url}/{object}/{branch}/{file:path}/{file:name}{line-block:start}#L{line:start}-{line:end}{line-block:end}".into(),
                file_at_commit: "{remote:url}/{object}/{commit}/{file:path}/{file:name}{line-block:start}#L{line:start}-{line:end}{line-block:end}".into(),
                commit: "{remote:url}/commit/{commit}".into(),
            },
            Self::BitbucketCloud => UrlTemplates {
                file_at_branch: "{remote:url}/src/{branch}/{file:path}/{file:name}{line-block:start}#lines-{line:start}:{line:end}{line-block:end}".into(),
                file_at_commit: "{remote:url}/src/{commit}/{file:path}/{file:name}{line-block:start}#lines-{line:start}:{line:end}{line-block:end}".into(),
                commit: "{remote:url}/commits/{commit}".into(),
            },
            Self::BitbucketServer => UrlTemplates {
                file_at_branch: "{remote:url:protocol}://{remote:url:host}/projects/{remote:url:path:0}/repos/{remote:url:path:1}/browse/{file:path}/{file:name}?at=refs/heads/{branch}{line-block:start}#{line:start}-{line:end}{line-block:end}".into(),
                file_at_commit: "{remote:url:protocol}://{remote:url:host}/projects/{remote:url:path:0}/repos/{remote:url:path:1}/browse/{file:path}/{file:name}?at={commit}{line-block:start}#{line:start}-{line:end}{line-block:end}".into(),
                commit: "{remote:url:protocol}://{remote:url:host}/projects/{remote:url:path:0}/repos/{remote:url:path:1}/commits/{commit}".into(),
            },
            Self::Codeberg | Self::Forgejo => UrlTemplates {
                file_at_branch: "{remote:url}/src/branch/{branch}/{file:path}/{file:name}{line-block:start}#L{line:start}-L{line:end}{line-block:end}".into(),
                file_at_commit: "{remote:url}/src/commit/{commit}/{file:path}/{file:name}{line-block:start}#L{line:start}-L{line:end}{line-block:end}".into(),
                commit: "{remote:url}/commit/{commit}".into(),
            },
            Self::Gitea | Self::Gogs => UrlTemplates {
                file_at_branch: "{remote:url}/src/{branch}/{file:path}/{file:name}{line-block:start}#L{line:start}-L{line:end}{line-block:end}".into(),
                file_at_commit: "{remote:url}/src/{commit}/{file:path}/{file:name}{line-block:start}#L{line:start}-L{line:end}{line-block:end}".into(),
                commit: "{remote:url}/commit/{commit}".into(),
            },
            Self::SourceHut => UrlTemplates {
                file_at_branch: "{remote:url}/tree/{branch}/item/{file:path}/{file:name}{line-block:start}#L{line:start}{line-block:end}".into(),
                file_at_commit: "{remote:url}/tree/{commit}/item/{file:path}/{file:name}{line-block:start}#L{line:start}{line-block:end}".into(),
                commit: "{remote:url}/tree/{commit}".into(),
            },
            Self::Gerrit => UrlTemplates {
                file_at_branch: "{remote:url:protocol}://{remote:url:host}/plugins/gitiles/{remote:url:path}/+/refs/heads/{branch}/{file:path}/{file:name}{line-block:start}#{line:start}{line-block:end}".into(),
                file_at_commit: "{remote:url:protocol}://{remote:url:host}/plugins/gitiles/{remote:url:path}/+/{commit}/{file:path}/{file:name}{line-block:start}#{line:start}{line-block:end}".into(),
                commit: "{remote:url:protocol}://{remote:url:host}/plugins/gitiles/{remote:url:path}/+/{commit}".into(),
            },
            Self::Azure => return None,
            Self::Custom => UrlTemplates {
                file_at_branch: settings.custom_file_at_branch.clone(),
                file_at_commit: settings.custom_file_at_commit.clone(),
                commit: settings.custom_commit.clone(),
            },
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UrlTemplates {
    pub file_at_branch: String,
    pub file_at_commit: String,
    pub commit: String,
}

/// Parsed HTTP(S) base URL of a Git remote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteUrl {
    pub scheme: String,
    pub host: String,
    /// Path without leading/trailing slashes or `.git` suffix (e.g. `l0ng-ai/openidea`).
    pub path: String,
}

impl RemoteUrl {
    /// Parses an SSH, Git, or HTTP(S) remote URL into a normalised web base URL.
    pub fn parse(raw: &str, force_https: bool) -> Option<Self> {
        let u = raw
            .trim()
            .trim_end_matches('/')
            .strip_suffix(".git")
            .unwrap_or_else(|| raw.trim().trim_end_matches('/'));
        let u = u.trim_end_matches('/');
        if u.is_empty() {
            return None;
        }
        let (raw_scheme, host, path) = if let Some((scheme, rest)) = u.split_once("://") {
            let scheme = scheme.to_ascii_lowercase();
            if !matches!(scheme.as_str(), "http" | "https" | "ssh" | "git") {
                return None;
            }
            let (authority, path) = rest.split_once('/')?;
            let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
            let host = if let Some(v6) = host_port.strip_prefix('[') {
                format!("[{}]", v6.split(']').next()?)
            } else {
                host_port.split(':').next()?.to_owned()
            };
            (scheme, host, path)
        } else {
            // SCP-like syntax: `[user@]host:path` (not a Windows drive `C:\...`).
            let (left, path) = u.split_once(':')?;
            if path.starts_with('/') || path.starts_with('\\') || left.contains(['/', '\\']) {
                return None;
            }
            let host = left.rsplit_once('@').map_or(left, |(_, h)| h);
            if host.len() <= 1 {
                return None;
            }
            ("https".to_owned(), host.to_owned(), path)
        };
        if host.is_empty() {
            return None;
        }
        let mut host = host;
        let mut path = path.trim_matches('/').to_owned();
        // Azure DevOps SSH: `git@ssh.dev.azure.com:v3/org/project/repo` → `dev.azure.com/org/project/_git/repo`.
        if host.eq_ignore_ascii_case("ssh.dev.azure.com") {
            host = "dev.azure.com".to_owned();
            if let Some(rest) = path.strip_prefix("v3/") {
                path = rest.to_owned();
            }
        }
        if path.is_empty() {
            return None;
        }
        let scheme = if force_https || raw_scheme != "http" {
            "https".into()
        } else {
            "http".into()
        };
        Some(Self { scheme, host, path })
    }

    pub fn base_url(&self) -> String {
        format!("{}://{}/{}", self.scheme, self.host, self.path)
    }
}

/// 1-based inclusive line range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineSelection {
    pub start: u32,
    pub end: u32,
}

impl LineSelection {
    pub fn new(start: u32, end: u32) -> Self {
        Self {
            start: start.max(1),
            end: end.max(start.max(1)),
        }
    }

    /// Converts 0-based half-open editor ranges to a 1-based inclusive line selection.
    pub fn from_ranges(ranges: &[Range]) -> Option<Self> {
        let r = ranges.first()?;
        let start = r.start.line + 1;
        // If a multi-line selection ends at column 0 of the next line, don't include that trailing line.
        let end_line = if r.end.line > r.start.line && r.end.character == 0 {
            r.end.line
        } else {
            r.end.line + 1
        };
        Some(Self::new(start, end_line))
    }
}

/// A file or directory inside the repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoFile {
    /// Directory path relative to repository root (empty when in root).
    pub dir_path: String,
    /// File or directory name (empty for repository root).
    pub name: String,
    pub is_directory: bool,
    pub is_root: bool,
}

impl RepoFile {
    pub fn root() -> Self {
        Self {
            dir_path: String::new(),
            name: String::new(),
            is_directory: true,
            is_root: true,
        }
    }

    pub fn from_rel_path(rel: &str, is_directory: bool) -> Self {
        let rel = rel.trim().trim_matches('/');
        if rel.is_empty() {
            return Self::root();
        }
        match rel.rsplit_once('/') {
            Some((dir, name)) => Self {
                dir_path: dir.to_owned(),
                name: name.to_owned(),
                is_directory,
                is_root: false,
            },
            None => Self {
                dir_path: String::new(),
                name: rel.to_owned(),
                is_directory,
                is_root: false,
            },
        }
    }

    pub fn full_path(&self) -> String {
        if self.is_root {
            String::new()
        } else if self.dir_path.is_empty() {
            self.name.clone()
        } else {
            format!("{}/{}", self.dir_path, self.name)
        }
    }

    pub fn display_name(&self, repo_root: &str) -> String {
        if !self.name.is_empty() {
            self.name.clone()
        } else {
            repo_root
                .trim_end_matches(['/', '\\'])
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or("repository")
                .to_owned()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkTarget {
    FileAtCommit {
        file: RepoFile,
        branch: String,
        commit: String,
        lines: Option<LineSelection>,
    },
    FileAtBranch {
        file: RepoFile,
        branch: String,
        lines: Option<LineSelection>,
    },
    Commit {
        commit: String,
        branch: String,
    },
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub platform: String,
    pub remote_name: String,
    pub fallback_branch: String,
    pub check_commit_on_remote: bool,
    pub force_https: bool,
    pub custom_file_at_branch: String,
    pub custom_file_at_commit: String,
    pub custom_commit: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            platform: "auto".into(),
            remote_name: "origin".into(),
            fallback_branch: "main".into(),
            check_commit_on_remote: true,
            force_https: true,
            custom_file_at_branch:
                "{remote:url}/{object}/{branch}/{file:path}/{file:name}{line-block:start}#L{line:start}-L{line:end}{line-block:end}"
                    .into(),
            custom_file_at_commit:
                "{remote:url}/{object}/{commit}/{file:path}/{file:name}{line-block:start}#L{line:start}-L{line:end}{line-block:end}"
                    .into(),
            custom_commit: "{remote:url}/commit/{commit}".into(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Self::default();
        if let Some(v) = host::setting_string("platform")
            && !v.trim().is_empty()
        {
            s.platform = v;
        }
        if let Some(v) = host::setting_string("remoteName")
            && !v.trim().is_empty()
        {
            s.remote_name = v;
        }
        if let Some(v) = host::setting_string("fallbackBranch")
            && !v.trim().is_empty()
        {
            s.fallback_branch = v;
        }
        if let Some(v) = host::setting_bool("checkCommitOnRemote") {
            s.check_commit_on_remote = v;
        }
        if let Some(v) = host::setting_bool("forceHttps") {
            s.force_https = v;
        }
        if let Some(v) = host::setting_string("customFileAtBranchTemplate")
            && !v.trim().is_empty()
        {
            s.custom_file_at_branch = v;
        }
        if let Some(v) = host::setting_string("customFileAtCommitTemplate")
            && !v.trim().is_empty()
        {
            s.custom_file_at_commit = v;
        }
        if let Some(v) = host::setting_string("customCommitTemplate")
            && !v.trim().is_empty()
        {
            s.custom_commit = v;
        }
        s
    }
}

/// Percent-encodes a URL path segment (matches Guava's `urlPathSegmentEscaper`).
pub fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        let unreserved = b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'-' | b'.'
                    | b'_'
                    | b'~'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'@'
            );
        if unreserved {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Percent-encodes a query parameter value (`/` and `&` are encoded).
fn encode_query(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn encode_slash_separated(s: &str) -> String {
    s.split('/')
        .filter(|p| !p.is_empty())
        .map(encode_segment)
        .collect::<Vec<_>>()
        .join("/")
}

/// Collapses duplicate slashes outside the `scheme://` prefix.
fn collapse_slashes(url: &str) -> String {
    let (prefix, rest) = match url.split_once("://") {
        Some((s, r)) => (format!("{s}://"), r),
        None => (String::new(), url),
    };
    let (path_part, suffix) = match rest.find(['?', '#']) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let mut cleaned = String::with_capacity(path_part.len());
    let mut prev_slash = false;
    for ch in path_part.chars() {
        if ch == '/' {
            if !prev_slash {
                cleaned.push('/');
            }
            prev_slash = true;
        } else {
            prev_slash = false;
            cleaned.push(ch);
        }
    }
    let cleaned = cleaned.trim_end_matches('/');
    format!("{prefix}{cleaned}{suffix}")
}

fn strip_line_block(template: &str) -> String {
    let mut out = template.to_owned();
    while let Some(start) = out.find("{line-block:start}") {
        if let Some(end_rel) = out[start..].find("{line-block:end}") {
            let end = start + end_rel + "{line-block:end}".len();
            out.replace_range(start..end, "");
        } else {
            break;
        }
    }
    out
}

fn remove_unmatched_placeholders(template: &str) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        if let Some(end) = rest[start..].find('}') {
            rest = &rest[start + end + 1..];
        } else {
            out.push_str(&rest[start..]);
            return out;
        }
    }
    out.push_str(rest);
    out
}

fn short_sha(commit: &str) -> &str {
    &commit[..commit.len().min(8)]
}

/// Generates the remote URL for `target` on `platform`.
pub fn build_url(
    platform: Platform,
    remote: &RemoteUrl,
    target: &LinkTarget,
    settings: &Settings,
) -> String {
    if platform == Platform::Azure {
        return build_azure_url(remote, target);
    }
    let mut remote = remote.clone();
    if platform == Platform::BitbucketServer
        && let Some(stripped) = remote.path.strip_prefix("scm/")
    {
        remote.path = stripped.to_owned();
    }
    let templates = platform
        .templates(settings)
        .expect("non-Azure platform has templates");
    let escape_branch = |branch: &str| {
        if platform == Platform::BitbucketServer {
            encode_query(branch)
        } else {
            encode_slash_separated(branch)
        }
    };

    let mut url = match target {
        LinkTarget::FileAtBranch {
            file,
            branch,
            lines,
        } => {
            let t = apply_file(&templates.file_at_branch, file);
            let t = t.replace("{branch}", &escape_branch(branch));
            apply_lines(&t, *lines, file.is_directory)
        }
        LinkTarget::FileAtCommit {
            file,
            branch,
            commit,
            lines,
        } => {
            let t = apply_file(&templates.file_at_commit, file);
            let t = t
                .replace("{commit}", commit)
                .replace("{commit:short}", short_sha(commit));
            let t = t.replace("{branch}", &escape_branch(branch));
            apply_lines(&t, *lines, file.is_directory)
        }
        LinkTarget::Commit { commit, branch } => templates
            .commit
            .replace("{commit}", commit)
            .replace("{commit:short}", short_sha(commit))
            .replace("{branch}", &escape_branch(branch)),
    };

    url = url
        .replace("{remote:url:protocol}", &remote.scheme)
        .replace("{remote:url:host}", &remote.host)
        .replace("{remote:url:path}", &remote.path)
        .replace("{remote:url}", &remote.base_url());

    let segments: Vec<&str> = remote.path.split('/').collect();
    for i in 0..10 {
        let token = format!("{{remote:url:path:{i}}}");
        if url.contains(&token) {
            url = url.replace(&token, segments.get(i).copied().unwrap_or(""));
        }
    }

    let url = remove_unmatched_placeholders(&url);
    collapse_slashes(&url)
}

fn apply_file(template: &str, file: &RepoFile) -> String {
    let object = if file.is_directory { "tree" } else { "blob" };
    let file_name = if file.is_root {
        String::new()
    } else {
        encode_segment(&file.name)
    };
    let file_path = encode_slash_separated(&file.dir_path);
    template
        .replace("{object}", object)
        .replace("{file:name}", &file_name)
        .replace("{file:path}", &file_path)
}

fn apply_lines(template: &str, lines: Option<LineSelection>, is_directory: bool) -> String {
    match lines {
        Some(sel) if !is_directory => template
            .replace("{line-block:start}", "")
            .replace("{line-block:end}", "")
            .replace("{line:start}", &sel.start.to_string())
            .replace("{line:end}", &sel.end.to_string()),
        _ => strip_line_block(template),
    }
}

fn build_azure_url(remote: &RemoteUrl, target: &LinkTarget) -> String {
    let mut parts: Vec<&str> = remote.path.split('/').filter(|s| !s.is_empty()).collect();
    if !parts.contains(&"_git") {
        let idx = if parts.len() >= 3 {
            2
        } else {
            1.min(parts.len())
        };
        parts.insert(idx, "_git");
    }
    let base = format!("{}://{}/{}", remote.scheme, remote.host, parts.join("/"));
    match target {
        LinkTarget::Commit { commit, .. } => format!("{base}/commit/{commit}"),
        LinkTarget::FileAtCommit {
            file,
            commit,
            lines,
            ..
        } => azure_file_url(&base, &format!("GC{commit}"), file, *lines),
        LinkTarget::FileAtBranch {
            file,
            branch,
            lines,
        } => azure_file_url(&base, &format!("GB{branch}"), file, *lines),
    }
}

fn azure_file_url(
    base: &str,
    version: &str,
    file: &RepoFile,
    lines: Option<LineSelection>,
) -> String {
    let raw_path = if file.is_root {
        "/".to_owned()
    } else {
        file.full_path()
    };
    let mut url = format!(
        "{base}?version={}&path={}",
        encode_query(version),
        encode_query(&raw_path)
    );
    if let Some(sel) = lines
        && !file.is_directory
    {
        url.push_str(&format!(
            "&line={}&lineEnd={}&lineStartColumn=1&lineEndColumn=1",
            sel.start,
            sel.end + 1
        ));
    }
    url
}

/// Picks the remote to use: the upstream's remote if tracked, else
/// `settings.remote_name` (`origin`), else the first configured remote.
pub fn select_remote<'a>(
    remotes: &'a [(String, String)],
    status: &RepositoryStatus,
    preferred: &str,
) -> Option<&'a (String, String)> {
    if let Some(up) = &status.upstream
        && let Some((remote_name, _)) = up.split_once('/')
        && let Some(found) = remotes.iter().find(|(n, _)| n == remote_name)
    {
        return Some(found);
    }
    remotes
        .iter()
        .find(|(n, _)| n == preferred)
        .or_else(|| remotes.first())
}

/// Whether `HEAD` is known to be pushed to the remote. When `check_commit_on_remote`
/// is on and the platform expects commits to be reachable from the remote
/// branch, requires an upstream with `ahead == 0` (or a detached `HEAD`).
pub fn resolve_commit(
    status: &RepositoryStatus,
    platform: Platform,
    settings: &Settings,
) -> Option<String> {
    let commit = status.commit.clone()?;
    if !platform.commits_reachable_from_remote() || !settings.check_commit_on_remote {
        return Some(commit);
    }
    if status.branch.is_none() {
        return Some(commit);
    }
    (status.upstream.is_some() && status.ahead == 0).then_some(commit)
}

pub fn resolve_branch(status: &RepositoryStatus, settings: &Settings) -> String {
    status
        .branch
        .clone()
        .unwrap_or_else(|| settings.fallback_branch.clone())
}

fn normalize_slashes(path: &str) -> String {
    path.replace('\\', "/")
}

fn relative_to_root(repo_root: &str, path: &str) -> Option<String> {
    let root = normalize_slashes(repo_root);
    let root = root.trim_end_matches('/');
    let p = normalize_slashes(path);
    let p = p.trim_end_matches('/');
    if p == root {
        return Some(String::new());
    }
    p.strip_prefix(root)?.strip_prefix('/').map(str::to_owned)
}

fn is_hex_sha(s: &str) -> bool {
    let s = s.trim();
    (7..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Resolves the target (`LinkTarget`) and its Markdown label from the command,
/// optional argument, and focused editor.
fn resolve_context(
    cmd: &str,
    arg: Option<&str>,
    ctx: &Invocation,
    project: &Project,
    status: &RepositoryStatus,
    platform: Platform,
    settings: &Settings,
) -> Result<(LinkTarget, String), Error> {
    let branch = resolve_branch(status, settings);
    let commit_cmd = matches!(cmd, "open-commit" | "copy-commit" | "copy-commit-markdown");
    if commit_cmd {
        let sha = match arg.map(str::trim).filter(|a| is_hex_sha(a)) {
            Some(sha) => sha.to_owned(),
            None => status
                .commit
                .clone()
                .ok_or("repository has no commits yet")?,
        };
        let label = short_sha(&sha).to_owned();
        return Ok((
            LinkTarget::Commit {
                commit: sha,
                branch,
            },
            label,
        ));
    }

    // If a raw commit SHA was passed as argument (e.g. from a commit action), link to the commit.
    if let Some(a) = arg
        .map(str::trim)
        .filter(|a| is_hex_sha(a) && !a.contains(['/', '\\', '.']))
    {
        let sha = a.to_owned();
        let label = short_sha(&sha).to_owned();
        return Ok((
            LinkTarget::Commit {
                commit: sha,
                branch,
            },
            label,
        ));
    }

    let (abs_path, lines) = match arg.map(str::trim).filter(|a| !a.is_empty()) {
        // Explicit path (Project View or Editor Tab context menu): no line selection.
        Some(path) => (path.to_owned(), None),
        None => match ctx.editor() {
            Some(editor) => {
                let sels = editor.selections().unwrap_or_default();
                (editor.path(), LineSelection::from_ranges(&sels))
            }
            None => (status.root.clone(), None),
        },
    };

    let rel = relative_to_root(&status.root, &abs_path).ok_or_else(|| {
        Error::Invalid(format!("{abs_path} is outside repository {}", status.root))
    })?;

    let is_directory = if rel.is_empty() {
        true
    } else {
        match project.read_file(&abs_path) {
            Ok(_) => false,
            Err(Error::Failed(m)) if m.contains("not a file") => true,
            Err(_) => false,
        }
    };

    let file = RepoFile::from_rel_path(&rel, is_directory);
    let label = match lines.filter(|_| !file.is_directory) {
        Some(sel) if sel.start == sel.end => {
            format!("{}:{}", file.display_name(&status.root), sel.start)
        }
        Some(sel) => format!(
            "{}:{}-{}",
            file.display_name(&status.root),
            sel.start,
            sel.end
        ),
        None => file.display_name(&status.root),
    };
    let target = match resolve_commit(status, platform, settings) {
        Some(commit) => LinkTarget::FileAtCommit {
            file,
            branch,
            commit,
            lines,
        },
        None => LinkTarget::FileAtBranch {
            file,
            branch,
            lines,
        },
    };
    Ok((target, label))
}

fn current_project(ctx: &Invocation) -> Option<Project> {
    if let Some(p) = ctx.project() {
        return Some(p);
    }
    host::projects().ok()?.into_iter().next()
}

impl Plugin for GitLink {
    fn run_command(
        &mut self,
        id: &str,
        ctx: &Invocation,
        arg: Option<&str>,
    ) -> Result<Option<String>, Error> {
        if !matches!(
            id,
            "open"
                | "copy"
                | "copy-markdown"
                | "open-commit"
                | "copy-commit"
                | "copy-commit-markdown"
        ) {
            return Err(Error::NotFound(format!("command {id}")));
        }
        let project = current_project(ctx).ok_or("no project is open")?;
        let status = project
            .vcs_status()?
            .ok_or("the project is not in a git repository")?;
        let remotes = project.vcs_remotes()?;
        if remotes.is_empty() {
            return Err("the repository has no configured git remotes".into());
        }
        let settings = Settings::load();
        let (remote_name, raw_url) =
            select_remote(&remotes, &status, &settings.remote_name).ok_or("no git remote found")?;
        let remote = RemoteUrl::parse(raw_url, settings.force_https).ok_or_else(|| {
            format!("remote '{remote_name}' ({raw_url}) does not have a supported web URL")
        })?;
        let platform = Platform::from_setting(&settings.platform, &remote.host);
        let (target, label) =
            resolve_context(id, arg, ctx, &project, &status, platform, &settings)?;
        let url = build_url(platform, &remote, &target, &settings);

        match id {
            "open" | "open-commit" => {
                host::open_url(&url)?;
                Ok(Some(url))
            }
            "copy" | "copy-commit" => {
                host::copy_to_clipboard(&url)?;
                host::notify(
                    NotificationLevel::Info,
                    &format!("Copied {} link", platform.name()),
                    &url,
                );
                Ok(Some(url))
            }
            "copy-markdown" | "copy-commit-markdown" => {
                let md = format!("[{label}]({url})");
                host::copy_to_clipboard(&md)?;
                host::notify(
                    NotificationLevel::Info,
                    &format!("Copied {} Markdown link", platform.name()),
                    &md,
                );
                Ok(Some(md))
            }
            _ => unreachable!(),
        }
    }
}

register_plugin!(GitLink);

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::mock::MockHost;

    const COMMIT: &str = "b032a0707beac9a2f24b1b7d97ee4f7156de182c";

    fn file_java() -> RepoFile {
        RepoFile::from_rel_path("src/Foo.java", false)
    }

    fn dir_resources() -> RepoFile {
        RepoFile::from_rel_path("src/foo/resources", true)
    }

    fn lines() -> Option<LineSelection> {
        Some(LineSelection::new(10, 20))
    }

    #[test]
    fn parses_ssh_https_and_git_remote_urls() {
        assert_eq!(
            RemoteUrl::parse("git@github.com:l0ng-ai/openidea.git", true),
            Some(RemoteUrl {
                scheme: "https".into(),
                host: "github.com".into(),
                path: "l0ng-ai/openidea".into()
            })
        );
        assert_eq!(
            RemoteUrl::parse(
                "ssh://git@gitlab.example.com:2222/group/sub/repo.git/",
                true
            ),
            Some(RemoteUrl {
                scheme: "https".into(),
                host: "gitlab.example.com".into(),
                path: "group/sub/repo".into(),
            })
        );
        assert_eq!(
            RemoteUrl::parse("http://gitea.local/org/app.git", false),
            Some(RemoteUrl {
                scheme: "http".into(),
                host: "gitea.local".into(),
                path: "org/app".into()
            })
        );
        assert_eq!(
            RemoteUrl::parse("http://gitea.local/org/app.git", true),
            Some(RemoteUrl {
                scheme: "https".into(),
                host: "gitea.local".into(),
                path: "org/app".into()
            })
        );
        assert_eq!(
            RemoteUrl::parse("git@ssh.dev.azure.com:v3/company/project/repo", true),
            Some(RemoteUrl {
                scheme: "https".into(),
                host: "dev.azure.com".into(),
                path: "company/project/repo".into(),
            })
        );
        assert_eq!(RemoteUrl::parse("/local/path/repo.git", true), None);
    }

    #[test]
    fn detects_platforms_from_host() {
        assert_eq!(Platform::detect("github.com"), Platform::GitHub);
        assert_eq!(Platform::detect("gitlab.com"), Platform::GitLab);
        assert_eq!(
            Platform::detect("gitlab.company.internal"),
            Platform::GitLab
        );
        assert_eq!(Platform::detect("bitbucket.org"), Platform::BitbucketCloud);
        assert_eq!(
            Platform::detect("bitbucket.company.internal"),
            Platform::BitbucketServer
        );
        assert_eq!(Platform::detect("codeberg.org"), Platform::Codeberg);
        assert_eq!(Platform::detect("forgejo.example.org"), Platform::Forgejo);
        assert_eq!(Platform::detect("gitea.example.org"), Platform::Gitea);
        assert_eq!(Platform::detect("gitee.com"), Platform::Gitee);
        assert_eq!(Platform::detect("gogs.example.org"), Platform::Gogs);
        assert_eq!(Platform::detect("dev.azure.com"), Platform::Azure);
        assert_eq!(Platform::detect("myorg.visualstudio.com"), Platform::Azure);
        assert_eq!(Platform::detect("git.sr.ht"), Platform::SourceHut);
        assert_eq!(
            Platform::detect("gerrit.googlesource.com"),
            Platform::Gerrit
        );
    }

    #[test]
    fn github_urls_match_gitlink_expectations() {
        let remote = RemoteUrl::parse("https://github.com/my/repo", true).unwrap();
        let s = Settings::default();
        assert_eq!(
            build_url(
                Platform::GitHub,
                &remote,
                &LinkTarget::FileAtBranch {
                    file: file_java(),
                    branch: "master".into(),
                    lines: lines()
                },
                &s
            ),
            "https://github.com/my/repo/blob/master/src/Foo.java#L10-L20"
        );
        assert_eq!(
            build_url(
                Platform::GitHub,
                &remote,
                &LinkTarget::FileAtBranch {
                    file: file_java(),
                    branch: "feature/ticket-23".into(),
                    lines: lines()
                },
                &s
            ),
            "https://github.com/my/repo/blob/feature/ticket-23/src/Foo.java#L10-L20"
        );
        assert_eq!(
            build_url(
                Platform::GitHub,
                &remote,
                &LinkTarget::FileAtBranch {
                    file: file_java(),
                    branch: "feature/ticket 23".into(),
                    lines: lines()
                },
                &s
            ),
            "https://github.com/my/repo/blob/feature/ticket%2023/src/Foo.java#L10-L20"
        );
        assert_eq!(
            build_url(
                Platform::GitHub,
                &remote,
                &LinkTarget::FileAtBranch {
                    file: RepoFile::from_rel_path("src/foo bar baz/images/my-image.png", false),
                    branch: "master".into(),
                    lines: None,
                },
                &s
            ),
            "https://github.com/my/repo/blob/master/src/foo%20bar%20baz/images/my-image.png"
        );
        assert_eq!(
            build_url(
                Platform::GitHub,
                &remote,
                &LinkTarget::FileAtCommit {
                    file: file_java(),
                    branch: "main".into(),
                    commit: COMMIT.into(),
                    lines: lines(),
                },
                &s
            ),
            format!("https://github.com/my/repo/blob/{COMMIT}/src/Foo.java#L10-L20")
        );
        assert_eq!(
            build_url(
                Platform::GitHub,
                &remote,
                &LinkTarget::FileAtCommit {
                    file: dir_resources(),
                    branch: "main".into(),
                    commit: COMMIT.into(),
                    lines: None,
                },
                &s
            ),
            format!("https://github.com/my/repo/tree/{COMMIT}/src/foo/resources")
        );
        assert_eq!(
            build_url(
                Platform::GitHub,
                &remote,
                &LinkTarget::FileAtCommit {
                    file: RepoFile::root(),
                    branch: "main".into(),
                    commit: COMMIT.into(),
                    lines: None,
                },
                &s
            ),
            format!("https://github.com/my/repo/tree/{COMMIT}")
        );
        assert_eq!(
            build_url(
                Platform::GitHub,
                &remote,
                &LinkTarget::Commit {
                    commit: COMMIT.into(),
                    branch: "main".into()
                },
                &s
            ),
            format!("https://github.com/my/repo/commit/{COMMIT}")
        );
    }

    #[test]
    fn gitlab_bitbucket_forgejo_gitea_srht_and_gerrit_urls() {
        let s = Settings::default();
        let gl = RemoteUrl::parse("https://gitlab.com/group/sub/repo.git", true).unwrap();
        assert_eq!(
            build_url(
                Platform::GitLab,
                &gl,
                &LinkTarget::FileAtCommit {
                    file: file_java(),
                    branch: "main".into(),
                    commit: COMMIT.into(),
                    lines: lines(),
                },
                &s
            ),
            format!("https://gitlab.com/group/sub/repo/blob/{COMMIT}/src/Foo.java#L10-20")
        );

        let bb = RemoteUrl::parse("https://bitbucket.org/my/repo.git", true).unwrap();
        assert_eq!(
            build_url(
                Platform::BitbucketCloud,
                &bb,
                &LinkTarget::FileAtCommit {
                    file: file_java(),
                    branch: "main".into(),
                    commit: COMMIT.into(),
                    lines: lines(),
                },
                &s
            ),
            format!("https://bitbucket.org/my/repo/src/{COMMIT}/src/Foo.java#lines-10:20")
        );
        assert_eq!(
            build_url(
                Platform::BitbucketCloud,
                &bb,
                &LinkTarget::Commit {
                    commit: COMMIT.into(),
                    branch: "main".into()
                },
                &s
            ),
            format!("https://bitbucket.org/my/repo/commits/{COMMIT}")
        );

        let bbs = RemoteUrl::parse("https://stash.example.com/scm/foo/bar.git", true).unwrap();
        assert_eq!(
            build_url(
                Platform::BitbucketServer,
                &bbs,
                &LinkTarget::FileAtBranch {
                    file: file_java(),
                    branch: "feature/ticket-23".into(),
                    lines: lines()
                },
                &s
            ),
            "https://stash.example.com/projects/foo/repos/bar/browse/src/Foo.java?at=refs/heads/feature%2Fticket-23#10-20"
        );
        assert_eq!(
            build_url(
                Platform::BitbucketServer,
                &bbs,
                &LinkTarget::FileAtCommit {
                    file: RepoFile::root(),
                    branch: "main".into(),
                    commit: COMMIT.into(),
                    lines: None,
                },
                &s
            ),
            format!("https://stash.example.com/projects/foo/repos/bar/browse?at={COMMIT}")
        );

        let cb = RemoteUrl::parse("https://codeberg.org/my/repo", true).unwrap();
        assert_eq!(
            build_url(
                Platform::Codeberg,
                &cb,
                &LinkTarget::FileAtCommit {
                    file: file_java(),
                    branch: "main".into(),
                    commit: COMMIT.into(),
                    lines: lines(),
                },
                &s
            ),
            format!("https://codeberg.org/my/repo/src/commit/{COMMIT}/src/Foo.java#L10-L20")
        );

        let srht = RemoteUrl::parse("https://git.sr.ht/~user/repo", true).unwrap();
        assert_eq!(
            build_url(
                Platform::SourceHut,
                &srht,
                &LinkTarget::FileAtCommit {
                    file: file_java(),
                    branch: "main".into(),
                    commit: COMMIT.into(),
                    lines: lines(),
                },
                &s
            ),
            format!("https://git.sr.ht/~user/repo/tree/{COMMIT}/item/src/Foo.java#L10")
        );

        let gerrit = RemoteUrl::parse("https://gerrit.example.com/my/repo", true).unwrap();
        assert_eq!(
            build_url(
                Platform::Gerrit,
                &gerrit,
                &LinkTarget::FileAtBranch {
                    file: file_java(),
                    branch: "main".into(),
                    lines: lines()
                },
                &s
            ),
            "https://gerrit.example.com/plugins/gitiles/my/repo/+/refs/heads/main/src/Foo.java#10"
        );
    }

    #[test]
    fn azure_urls_match_gitlink_expectations() {
        let s = Settings::default();
        let with_git =
            RemoteUrl::parse("https://dev.azure.com/ben-gibson/_git/test", true).unwrap();
        let without_git =
            RemoteUrl::parse("https://dev.azure.com/company/project/test", true).unwrap();
        assert_eq!(
            build_url(
                Platform::Azure,
                &with_git,
                &LinkTarget::FileAtBranch {
                    file: file_java(),
                    branch: "master".into(),
                    lines: lines()
                },
                &s
            ),
            "https://dev.azure.com/ben-gibson/_git/test?version=GBmaster&path=src%2FFoo.java&line=10&lineEnd=21&lineStartColumn=1&lineEndColumn=1"
        );
        assert_eq!(
            build_url(
                Platform::Azure,
                &with_git,
                &LinkTarget::FileAtCommit {
                    file: RepoFile::root(),
                    branch: "main".into(),
                    commit: COMMIT.into(),
                    lines: None,
                },
                &s
            ),
            format!("https://dev.azure.com/ben-gibson/_git/test?version=GC{COMMIT}&path=%2F")
        );
        assert_eq!(
            build_url(
                Platform::Azure,
                &without_git,
                &LinkTarget::Commit {
                    commit: COMMIT.into(),
                    branch: "main".into()
                },
                &s
            ),
            format!("https://dev.azure.com/company/project/_git/test/commit/{COMMIT}")
        );
    }

    #[test]
    fn commands_open_and_copy_links_with_unpushed_fallback() {
        let host = MockHost::install();
        host.add_file("src/lib.rs", "fn main() {}\n");
        host.open_editor(
            "/project/src/lib.rs",
            "rust",
            "fn main() {}\n// line 2\n",
            &[(0, 0, 1, 5)],
        );
        host.set_vcs_remotes(&[("origin", "git@github.com:l0ng-ai/openidea.git")]);
        host.set_vcs_status(Some(RepositoryStatus {
            root: "/project".into(),
            branch: Some("main".into()),
            commit: Some(COMMIT.into()),
            upstream: Some("origin/main".into()),
            ahead: 0,
            behind: 0,
            ..RepositoryStatus::default()
        }));

        // Pushed HEAD: uses the commit SHA permalink.
        let url = GitLink
            .run_command("copy", &host.invocation(), None)
            .unwrap()
            .unwrap();
        assert_eq!(
            url,
            format!("https://github.com/l0ng-ai/openidea/blob/{COMMIT}/src/lib.rs#L1-L2")
        );
        assert_eq!(host.clipboard().as_deref(), Some(url.as_str()));

        // Markdown link includes file name and 1-based line range.
        let md = GitLink
            .run_command("copy-markdown", &host.invocation(), None)
            .unwrap()
            .unwrap();
        assert_eq!(
            md,
            format!(
                "[lib.rs:1-2](https://github.com/l0ng-ai/openidea/blob/{COMMIT}/src/lib.rs#L1-L2)"
            )
        );
        assert_eq!(host.clipboard().as_deref(), Some(md.as_str()));

        // Open in browser calls `open_url`.
        GitLink
            .run_command("open", &host.invocation(), None)
            .unwrap();
        assert_eq!(host.opened_urls(), [url]);

        // Unpushed commit (`ahead > 0`): falls back to the branch ref so the link does not 404.
        host.set_vcs_status(Some(RepositoryStatus {
            root: "/project".into(),
            branch: Some("feat/gitlink".into()),
            commit: Some(COMMIT.into()),
            upstream: Some("origin/feat/gitlink".into()),
            ahead: 1,
            behind: 0,
            ..RepositoryStatus::default()
        }));
        let branch_url = GitLink
            .run_command("copy", &host.invocation(), None)
            .unwrap()
            .unwrap();
        assert_eq!(
            branch_url,
            "https://github.com/l0ng-ai/openidea/blob/feat/gitlink/src/lib.rs#L1-L2"
        );

        // Project View context menu argument (a file or commit hash).
        let file_url = GitLink
            .run_command("copy", &host.invocation(), Some("/project/src/lib.rs"))
            .unwrap()
            .unwrap();
        assert_eq!(
            file_url,
            "https://github.com/l0ng-ai/openidea/blob/feat/gitlink/src/lib.rs"
        );

        let commit_md = GitLink
            .run_command("copy-commit-markdown", &host.invocation(), None)
            .unwrap()
            .unwrap();
        assert_eq!(
            commit_md,
            format!("[b032a070](https://github.com/l0ng-ai/openidea/commit/{COMMIT})")
        );
    }
}
