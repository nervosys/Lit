//! System use notification.
//!
//! NIST SP 800-171r3 `03.01.09` requires that users be shown a system use
//! notification before they are granted access, with the wording set by the
//! organization. Lit cannot supply that wording — it depends on the system
//! owner and, for CUI systems, on the language their legal counsel approves.
//!
//! What Lit provides is the mechanism: a banner served before authentication,
//! sourced from an operator-supplied file, with a conservative default that is
//! explicitly marked as requiring review.

use std::fs;
use std::path::Path;

/// The placeholder banner. It is deliberately worded so that shipping it
/// unmodified is visible in an assessment rather than quietly non-compliant.
pub const DEFAULT_BANNER: &str = "\
NOTICE TO USERS

This is a Lit version control server. It is for authorized use only. By using
this system you consent to monitoring and recording of your activity. Evidence
of unauthorized use may be provided to law enforcement.

[This is Lit's default placeholder text. NIST SP 800-171r3 03.01.09 requires a
system use notification whose wording is set by the system owner. Replace it by
setting server.banner_path in your Lit configuration.]";

/// A system use notification, and whether it is still the placeholder.
#[derive(Debug, Clone)]
pub struct Banner {
    pub text: String,
    /// True when the operator has supplied their own wording.
    pub customized: bool,
}

impl Banner {
    /// Load the banner from `path`, falling back to the placeholder.
    ///
    /// A configured path that cannot be read is an error rather than a silent
    /// fallback: an operator who set the path meant for that text to be shown,
    /// and quietly serving the placeholder instead would misrepresent the
    /// system's posture.
    pub fn load(path: Option<&Path>) -> Result<Self, String> {
        match path {
            None => Ok(Banner {
                text: DEFAULT_BANNER.to_string(),
                customized: false,
            }),
            Some(p) => {
                let text = fs::read_to_string(p)
                    .map_err(|e| format!("Failed to read banner file {}: {}", p.display(), e))?;
                if text.trim().is_empty() {
                    return Err(format!("Banner file {} is empty", p.display()));
                }
                Ok(Banner {
                    text,
                    customized: true,
                })
            }
        }
    }

    /// The banner as a JSON body, served before authentication.
    pub fn to_json(&self) -> String {
        serde_json::json!({
            "status": "ok",
            "banner": self.text,
            "customized": self.customized,
            "control": "03.01.09",
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn the_default_banner_is_marked_as_a_placeholder() {
        let banner = Banner::load(None).unwrap();
        assert!(!banner.customized);
        assert!(banner.text.contains("03.01.09"));
    }

    #[test]
    fn an_operator_banner_is_used_verbatim_and_marked_customized() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("banner.txt");
        fs::write(&path, "AUTHORIZED USE ONLY — Contoso Federal").unwrap();
        let banner = Banner::load(Some(&path)).unwrap();
        assert_eq!(banner.text, "AUTHORIZED USE ONLY — Contoso Federal");
        assert!(banner.customized);
    }

    #[test]
    fn a_missing_or_empty_banner_file_is_an_error_not_a_silent_fallback() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("absent.txt");
        assert!(Banner::load(Some(&missing)).is_err());

        let empty = dir.path().join("empty.txt");
        fs::write(&empty, "   \n").unwrap();
        assert!(Banner::load(Some(&empty)).is_err());
    }
}
