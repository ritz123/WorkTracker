use crate::models::{APP_RELEASES_URL, APP_REPO_URL, APP_VERSION};

#[derive(Debug, Clone)]
pub struct UpdateCheckResult {
    pub ok: bool,
    pub current_version: String,
    pub remote_version: Option<String>,
    pub is_newer: bool,
    pub html_url: Option<String>,
    pub name: Option<String>,
    pub error_message: Option<String>,
}

struct GithubSlug {
    owner: String,
    repo: String,
}

fn parse_github_repo() -> Option<GithubSlug> {
    let url = APP_REPO_URL;
    let rest = url.strip_prefix("https://github.com/")?;
    let mut parts = rest.split('/');
    let owner = parts.next()?.trim().to_string();
    let repo = parts
        .next()?
        .trim()
        .trim_end_matches(".git")
        .to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(GithubSlug { owner, repo })
}

/// Strips a leading `v` and build metadata so `1.17.0` compares equal to `v1.17.0+build`.
fn normalize_version_tag(v: &str) -> String {
    let mut s = v.trim().trim_start_matches(['v', 'V']).to_string();
    if let Some(idx) = s.find('+') {
        s.truncate(idx);
    }
    s.trim().to_string()
}

fn semver_tuple(v: &str) -> (u32, u32, u32) {
    let normalized = normalize_version_tag(v);
    let core = normalized.split('-').next().unwrap_or("");
    let mut parts = core.split('.');
    let maj = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let min = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let pat = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    (maj, min, pat)
}

/// Returns `1` if `a` is newer than `b`, `-1` if older, `0` if equal on major.minor.patch.
pub fn compare_semver(a: &str, b: &str) -> i32 {
    let (am, ai, ap) = semver_tuple(a);
    let (bm, bi, bp) = semver_tuple(b);
    for (x, y) in [(am, bm), (ai, bi), (ap, bp)] {
        if x != y {
            return if x > y { 1 } else { -1 };
        }
    }
    0
}

fn fetch_latest_via_atom(owner: &str, repo: &str) -> Result<(String, Option<String>, Option<String>), String> {
    let url = format!("https://github.com/{owner}/{repo}/releases.atom");
    let mut response = ureq::get(&url).call().map_err(|e| e.to_string())?;
    let xml = response
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;

  let first_entry = xml
        .split("<entry>")
        .nth(1)
        .and_then(|chunk| chunk.split("</entry>").next())
        .ok_or_else(|| "No releases found in GitHub feed.".to_string())?;

    let title = extract_xml_tag(first_entry, "title")
        .ok_or_else(|| "Could not parse release version from feed.".to_string())?;
    let tag = normalize_version_tag(&title);
    if tag.is_empty() {
        return Err("Could not parse release version from feed.".into());
    }

    let html_url = extract_link_href(first_entry);
    Ok((tag, html_url, Some(title)))
}

fn extract_xml_tag(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = block.find(&open)? + open.len();
    let end = block[start..].find(&close)? + start;
    Some(block[start..end].trim().to_string())
}

fn extract_link_href(block: &str) -> Option<String> {
    block.split("href=\"").skip(1).find_map(|rest| {
        let url = rest.split('"').next()?;
        url.starts_with("https://github.com/")
            .then(|| url.to_string())
    })
}

fn fetch_latest_via_api(owner: &str, repo: &str) -> Result<(String, Option<String>, Option<String>), String> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let mut response = ureq::get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "weekly-work-tracker")
        .call()
        .map_err(|e| e.to_string())?;

    let status = response.status().as_u16();
    if status == 404 {
        return Err("no releases".into());
    }
    if !(200..300).contains(&status) {
        return Err(format!("GitHub API HTTP {status}"));
    }

    let body_text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    let body: serde_json::Value = serde_json::from_str(&body_text).map_err(|e| e.to_string())?;
    let tag = normalize_version_tag(body["tag_name"].as_str().unwrap_or_default());
    let html_url = body["html_url"].as_str().map(str::to_string);
    let name = body["name"].as_str().map(str::to_string).or_else(|| Some(tag.clone()));
    Ok((tag, html_url, name))
}

/// Compares this build to the newest GitHub release (API with Atom fallback).
pub fn check_for_updates() -> UpdateCheckResult {
    let current_version = APP_VERSION.to_string();
    let Some(slug) = parse_github_repo() else {
        return UpdateCheckResult {
            ok: false,
            current_version,
            remote_version: None,
            is_newer: false,
            html_url: None,
            name: None,
            error_message: Some("No GitHub repository URL configured.".into()),
        };
    };

    let fetch = match fetch_latest_via_api(&slug.owner, &slug.repo) {
        Ok(v) => Ok(v),
        Err(ref api_err) if api_err == "no releases" => {
            return UpdateCheckResult {
                ok: true,
                current_version,
                remote_version: None,
                is_newer: false,
                html_url: Some(APP_RELEASES_URL.to_string()),
                name: None,
                error_message: None,
            };
        }
        Err(api_err) => fetch_latest_via_atom(&slug.owner, &slug.repo).map_err(|atom_err| {
            format!("{api_err}; feed fallback: {atom_err}")
        }),
    };

    match fetch {
        Err(message) => UpdateCheckResult {
            ok: false,
            current_version,
            remote_version: None,
            is_newer: false,
            html_url: Some(APP_RELEASES_URL.to_string()),
            name: None,
            error_message: Some(message),
        },
        Ok((tag, html_url, name)) => {
            let is_newer = compare_semver(&tag, &current_version) > 0;
            UpdateCheckResult {
                ok: true,
                current_version,
                remote_version: Some(tag.clone()),
                is_newer,
                html_url: html_url.or_else(|| Some(APP_RELEASES_URL.to_string())),
                name: name.or(Some(tag)),
                error_message: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_ordering() {
        assert_eq!(compare_semver("1.18.0", "1.17.0"), 1);
        assert_eq!(compare_semver("v1.17.0", "1.17.0"), 0);
        assert_eq!(compare_semver("1.16.9", "1.17.0"), -1);
    }
}