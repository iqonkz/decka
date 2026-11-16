//! Update check (ARCHITECTURE.md 8a): once a day and on request the agent asks GitHub for the latest release and only
//! tells the user; nothing is downloaded or installed. Only `tag_name`, `html_url` and `published_at` are read.

use std::time::Duration;

use serde::Serialize;

/// Where the latest release is asked for (built into the code).
pub const LATEST_URL: &str = "https://api.github.com/repos/iqonkz/decka/releases/latest";
/// An `html_url` is accepted only when it starts with this.
pub const RELEASES_PREFIX: &str = "https://github.com/iqonkz/decka/releases/";
/// The check is repeated this often.
pub const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
const TIMEOUT: Duration = Duration::from_secs(10);

/// What the latest release says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// `tag_name` without a leading `v`.
    pub version: String,
    pub url: String,
    pub published_at: String,
}

/// A newer release the user can download.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
    pub published_at: String,
}

/// What the window shows (`More → About`, the banner, the dot at `More`); mirrors `UpdateStatus` in shared/src/ipc.ts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// A newer release, if the last check found one.
    pub available: Option<UpdateInfo>,
    /// When the last check finished (ms since the epoch); `None` before the first one.
    pub checked_at: Option<u64>,
    /// The last check could not reach GitHub or did not understand the answer.
    pub failed: bool,
}

/// `1.2.3` or `v1.2.3` -> `(1, 2, 3)`; a pre-release (`1.2.3-beta.1`) or anything else -> `None`.
pub fn parse_stable(version: &str) -> Option<(u64, u64, u64)> {
    let v = version.strip_prefix('v').unwrap_or(version);
    let mut it = v.split('.');
    let (a, b, c) = (it.next()?, it.next()?, it.next()?);
    if it.next().is_some() {
        return None;
    }
    let num = |s: &str| (!s.is_empty() && s.len() <= 9 && s.bytes().all(|x| x.is_ascii_digit())).then(|| s.parse::<u64>().ok()).flatten();
    Some((num(a)?, num(b)?, num(c)?))
}

/// Whether `latest` is a stable release newer than `current` (`0.10.0` > `0.9.9`; `1.0.0-beta.1` is skipped; `v0.2.0` = `0.2.0`).
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_stable(latest), parse_stable(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// Reads the answer of `releases/latest`: only `tag_name`, `html_url`, `published_at`; a pre-release or a draft, a foreign
/// `html_url` or a missing field gives `None`.
pub fn parse_release(body: &str) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    if v.get("prerelease").and_then(|p| p.as_bool()) == Some(true) || v.get("draft").and_then(|p| p.as_bool()) == Some(true) {
        return None;
    }
    let tag = v.get("tag_name")?.as_str()?;
    let url = v.get("html_url")?.as_str()?;
    let published = v.get("published_at").and_then(|p| p.as_str()).unwrap_or("");
    parse_stable(tag)?;
    if !url.starts_with(RELEASES_PREFIX) || url.len() > 300 {
        return None;
    }
    Some(Release { version: tag.strip_prefix('v').unwrap_or(tag).to_string(), url: url.to_string(), published_at: published.chars().take(40).collect() })
}

/// The release the user could move to: the answer is newer than `current`.
pub fn newer_release(body: &str, current: &str) -> Result<Option<UpdateInfo>, ()> {
    let release = parse_release(body).ok_or(())?;
    Ok(is_newer(&release.version, current).then_some(UpdateInfo { version: release.version, url: release.url, published_at: release.published_at }))
}

/// Asks `url` (normally `LATEST_URL`) and returns the body; any error -> `Err`. A `User-Agent` is required by the GitHub API.
pub async fn fetch_latest(url: &str) -> Result<String, ()> {
    let client = reqwest::Client::builder().timeout(TIMEOUT).user_agent(format!("decka-agent/{}", crate::VERSION)).build().map_err(|_| ())?;
    let response = client.get(url).send().await.map_err(|_| ())?;
    if !response.status().is_success() {
        return Err(());
    }
    response.text().await.map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANSWER: &str = include_str!("../../../shared/fixtures/github-release-latest.json");

    #[test]
    fn newer_follows_semver_not_text() {
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn prereleases_are_skipped_and_the_v_prefix_is_fine() {
        assert!(!is_newer("1.0.0-beta.1", "0.1.0"));
        assert!(!is_newer("2.0.0", "1.0.0-beta.1"), "an unknown current version never offers anything");
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("0.2.0", "v0.1.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }

    #[test]
    fn the_fixture_answer_is_read_by_its_three_fields() {
        let r = parse_release(ANSWER).expect("release");
        assert_eq!(r.version, "0.2.0");
        assert_eq!(r.url, "https://github.com/iqonkz/decka/releases/tag/v0.2.0");
        assert_eq!(r.published_at, "2026-12-01T10:00:00Z");
        assert_eq!(newer_release(ANSWER, "0.1.0").unwrap().unwrap().version, "0.2.0");
        assert_eq!(newer_release(ANSWER, "0.2.0").unwrap(), None);
        assert_eq!(newer_release(ANSWER, "0.3.0").unwrap(), None);
    }

    #[test]
    fn a_foreign_link_a_prerelease_and_junk_are_refused() {
        let foreign = ANSWER.replace("https://github.com/iqonkz/decka/releases/tag/v0.2.0", "https://evil.example/releases/tag/v0.2.0");
        assert!(parse_release(&foreign).is_none());
        let sneaky = ANSWER.replace("https://github.com/iqonkz/decka/releases/tag/v0.2.0", "https://github.com/iqonkz/decka-evil/releases/tag/v0.2.0");
        assert!(parse_release(&sneaky).is_none());
        let pre = ANSWER.replace("\"prerelease\": false", "\"prerelease\": true");
        assert!(parse_release(&pre).is_none());
        let beta = ANSWER.replace("\"v0.2.0\"", "\"v1.0.0-beta.1\"");
        assert!(parse_release(&beta).is_none());
        assert!(parse_release("not json").is_none());
        assert!(parse_release("{}").is_none());
        assert_eq!(newer_release("{}", "0.1.0"), Err(()));
    }
}
