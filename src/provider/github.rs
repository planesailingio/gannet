use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use ureq::Agent;
use ureq::http::Response;

use super::{Asset, Provider, Release};

const API_ROOT: &str = "https://api.github.com";

pub struct Github {
    agent: Agent,
    token: Option<String>,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

impl Github {
    pub fn new() -> Self {
        let config = Agent::config_builder().http_status_as_error(false).build();
        let token = ["GITHUB_TOKEN", "GH_TOKEN"]
            .iter()
            .find_map(|v| std::env::var(v).ok())
            .filter(|t| !t.is_empty());
        Github {
            agent: Agent::new_with_config(config),
            token,
        }
    }

    fn get(&self, url: &str) -> Result<Response<ureq::Body>> {
        let mut req = self
            .agent
            .get(url)
            .header("User-Agent", concat!("gannet/", env!("CARGO_PKG_VERSION")))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = &self.token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        req.call()
            .with_context(|| format!("request to {url} failed"))
    }

    fn fetch_release(&self, url: &str, what: &str) -> Result<Option<Release>> {
        let mut resp = self.get(url)?;
        match resp.status().as_u16() {
            200 => {
                let api: ApiRelease = resp
                    .body_mut()
                    .read_json()
                    .context("could not parse the GitHub API response")?;
                Ok(Some(Release {
                    tag: api.tag_name,
                    assets: api
                        .assets
                        .into_iter()
                        .map(|a| Asset {
                            name: a.name,
                            download_url: a.browser_download_url,
                            size: a.size,
                        })
                        .collect(),
                }))
            }
            404 => Ok(None),
            403 | 429 => {
                let remaining = resp
                    .headers()
                    .get("x-ratelimit-remaining")
                    .and_then(|v| v.to_str().ok());
                if remaining == Some("0") {
                    let hint = if self.token.is_some() {
                        "the GitHub rate limit for your token is exhausted; try again later"
                    } else {
                        "GitHub rate limit exceeded (60 requests/hour unauthenticated); set GITHUB_TOKEN to raise it to 5,000/hour"
                    };
                    bail!("{hint}");
                }
                bail!(
                    "GitHub refused the request for {what} (HTTP {})",
                    resp.status()
                );
            }
            code => bail!("GitHub API returned HTTP {code} for {what}"),
        }
    }
}

impl Default for Github {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for Github {
    fn id(&self) -> &'static str {
        "github"
    }

    fn latest_release(&self, owner: &str, repo: &str) -> Result<Release> {
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/releases/latest");
        self.fetch_release(&url, &format!("{owner}/{repo}"))?
            .with_context(|| {
                format!(
                    "no releases found for {owner}/{repo} — is the repository public, and does it publish (non-prerelease) releases? For prerelease-only projects, install a specific tag with {owner}/{repo}@<tag>"
                )
            })
    }

    fn release_by_tag(&self, owner: &str, repo: &str, tag: &str) -> Result<Release> {
        let url = |t: &str| format!("{API_ROOT}/repos/{owner}/{repo}/releases/tags/{t}");
        if let Some(release) = self.fetch_release(&url(tag), &format!("{owner}/{repo}@{tag}"))? {
            return Ok(release);
        }
        // Tolerate the v-prefix ambiguity: retry with it toggled.
        let toggled = match tag.strip_prefix('v') {
            Some(bare) => bare.to_string(),
            None => format!("v{tag}"),
        };
        if let Some(release) =
            self.fetch_release(&url(&toggled), &format!("{owner}/{repo}@{toggled}"))?
        {
            return Ok(release);
        }
        bail!("no release tagged '{tag}' (or '{toggled}') in {owner}/{repo}");
    }

    fn download(&self, asset: &Asset, dest: &Path) -> Result<()> {
        let mut resp = self.get(&asset.download_url)?;
        if !resp.status().is_success() {
            bail!(
                "download of {} failed with HTTP {}",
                asset.name,
                resp.status()
            );
        }
        let mut reader = resp.body_mut().with_config().limit(u64::MAX).reader();
        let mut file =
            File::create(dest).with_context(|| format!("could not create {}", dest.display()))?;
        std::io::copy(&mut reader, &mut file)
            .with_context(|| format!("download of {} was interrupted", asset.name))?;
        Ok(())
    }
}
