use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};
use lazy_static::lazy_static;
use regex::Regex;
use serde_derive::{Deserialize, Serialize};

const KOJIPKGS_URL: &str = "https://kojipkgs.fedoraproject.org/packages";

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct KojiBuildInfo {
    nvr: String,
    id: u64,
    kojipkgs_url_prefix: String,
    rpms: BTreeMap<String, Vec<String>>,
}

// This likely isn't right, need to use something more like hy_split_nevra() maybe or reimplement in Rust
fn split_nvr(pkg: &str) -> Result<(&str, &str, &str)> {
    let idx = pkg
        .rfind('-')
        .ok_or_else(|| anyhow::anyhow!("Invalid buildid, missing a '-'"))?;
    let (pkgver, rest) = pkg.split_at(idx);
    let rest = rest.strip_prefix("-").expect("-");
    let idx = pkgver
        .rfind('-')
        .ok_or_else(|| anyhow::anyhow!("Invalid buildid, missing pkgver '-'"))?;
    let (pkgname, version) = pkgver.split_at(idx);
    let version = version.strip_prefix("-").expect("-");
    if pkgname.is_empty() {
        anyhow::bail!("Invalid buildid with empty name");
    }
    if version.is_empty() {
        anyhow::bail!("Invalid buildid with empty version");
    }
    if rest.is_empty() {
        anyhow::bail!("Invalid buildid with empty release");
    }
    Ok((pkgname, version, rest))
}

fn get_kojipkgs_url_prefix(buildid: &str) -> Result<String> {
    let (name, version, release) = split_nvr(buildid)?;
    Ok(format!("{}/{}/{}/{}", KOJIPKGS_URL, name, version, release))
}

pub(crate) fn validate_buildid(s: &str) -> Result<()> {
    // None of this supports non-ASCII
    if let Some(c) = s.chars().find(|c| !c.is_ascii()) {
        bail!("Invalid non-ASCII character {} in buildid", c);
    }
    // Validating the first character is alphanumeric shuts down potential
    // special characters like `-` and `.` etc.
    match s.chars().next() {
        Some(c) => {
            if !c.is_ascii_alphanumeric() {
                bail!("Invalid alphanumeric character {} in buildid", c);
            }
        }
        None => {
            bail!("Invalid empty buildid");
        }
    }
    Ok(())
}

lazy_static! {
    static ref BUILDRE: Regex = Regex::new(r#"^BUILD: +([^ ]+) +\[(\d+)\]"#).unwrap();
}

fn scrape_koji_cli(output: &str) -> Result<KojiBuildInfo> {
    let mut r: KojiBuildInfo = Default::default();
    // Convenience so the client doesn't have to hardcode this
    let mut in_rpms = false;
    for line in output.lines() {
        if in_rpms {
            let p = Path::new(line.split_whitespace().next().expect("split"));
            let name = p
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Missing RPM name"))?;
            let name = name
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid RPM name"))?;
            let arch = p
                .parent()
                .map(|p| p.file_name())
                .flatten()
                .ok_or_else(|| anyhow::anyhow!("Missing RPM arch"))?;
            let arch = arch
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid RPM arch"))?;

            let v = r.rpms.entry(arch.to_string()).or_default();
            v.push(name.to_string());
        } else if let Some(m) = BUILDRE.captures(line) {
            r.nvr = m[1].to_string();
            r.id = str::parse(&m[2]).expect("parse u64");
        } else if line.starts_with("RPMs:") {
            in_rpms = true;
        }
    }
    if r.nvr.is_empty() {
        bail!("Failed to find BUILD");
    }
    if !in_rpms {
        bail!("Failed to find RPMs");
    }
    r.kojipkgs_url_prefix = get_kojipkgs_url_prefix(&r.nvr)?;
    Ok(r)
}

pub(crate) fn get_koji_build(buildid: &str) -> Result<KojiBuildInfo> {
    validate_buildid(buildid)?;
    let c = Command::new("koji")
        .arg("buildinfo")
        .arg(buildid)
        .stdout(std::process::Stdio::piped())
        .output()?;
    if !c.status.success() {
        anyhow::bail!("koji failed");
    }
    scrape_koji_cli(std::str::from_utf8(&c.stdout)?)
}

#[cfg(test)]
mod test {
    use super::*;

    const KOJI_OUTPUT: &str = include_str!("example-koji-output.txt");

    #[test]
    fn test_validate_buildid() -> Result<()> {
        validate_buildid("42")?;
        validate_buildid("rpm-ostree-2020.10-1.fc34")?;
        assert!(validate_buildid("").is_err());
        assert!(validate_buildid("-foo").is_err());
        assert!(validate_buildid("../bar.rpm").is_err());
        Ok(())
    }

    #[test]
    fn test_buildre() {
        let s = "BUILD: rpm-ostree-2020.10-1.fc34 [1657648]";
        assert!(BUILDRE.captures(s).is_some());
    }

    #[test]
    fn test_scrape_koji_cli() -> Result<()> {
        let r = scrape_koji_cli(KOJI_OUTPUT)?;
        assert_eq!(r.nvr, "rpm-ostree-2020.10-1.fc34");
        assert_eq!(r.id, 1657648);
        assert_eq!(r.rpms.len(), 7);
        assert_eq!(r.rpms["src"][0], "rpm-ostree-2020.10-1.fc34.src.rpm");
        assert_eq!(
            r.rpms["x86_64"][2],
            "rpm-ostree-libs-debuginfo-2020.10-1.fc34.x86_64.rpm"
        );
        Ok(())
    }
}
