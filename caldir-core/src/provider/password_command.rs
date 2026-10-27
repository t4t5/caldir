//! Password read from an external command.

use std::process::{ExitStatus, Stdio};

use serde::{Deserialize, Serialize};
use tokio::process::Command;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PasswordCommandError {
    #[error("failed to run password command `{program}`")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },

    #[error("password command `{program}` failed ({status})")]
    Failed { program: String, status: ExitStatus },

    #[error("password command `{program}` output is not UTF-8")]
    NotUtf8 { program: String },

    #[error("password command `{program}` printed nothing")]
    Empty { program: String },
}

/// Argv of a command whose stdout is the password, e.g. `["pass", "show", "caldav"]`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "Vec<String>")]
pub struct PasswordCommand(Vec<String>);

impl TryFrom<Vec<String>> for PasswordCommand {
    type Error = &'static str;

    fn try_from(argv: Vec<String>) -> Result<Self, Self::Error> {
        if argv.is_empty() {
            return Err("password command must not be empty");
        }
        Ok(Self(argv))
    }
}

impl PasswordCommand {
    /// Run the command and return its stdout minus one trailing newline.
    pub async fn run(&self) -> Result<String, PasswordCommandError> {
        let (program, args) = self.0.split_first().expect("argv is non-empty");
        let program_name = || program.clone();

        let output = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::inherit())
            .output()
            .await
            .map_err(|source| PasswordCommandError::Spawn {
                program: program_name(),
                source,
            })?;

        if !output.status.success() {
            return Err(PasswordCommandError::Failed {
                program: program_name(),
                status: output.status,
            });
        }

        let stdout =
            String::from_utf8(output.stdout).map_err(|_| PasswordCommandError::NotUtf8 {
                program: program_name(),
            })?;
        let password = trim_newline(&stdout);
        if password.is_empty() {
            return Err(PasswordCommandError::Empty {
                program: program_name(),
            });
        }

        Ok(password.to_string())
    }
}

fn trim_newline(s: &str) -> &str {
    let s = s.strip_suffix('\n').unwrap_or(s);
    s.strip_suffix('\r').unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(argv: &[&str]) -> PasswordCommand {
        PasswordCommand(argv.iter().map(|s| s.to_string()).collect())
    }

    #[derive(Debug, Deserialize)]
    struct Wrapper {
        cmd: PasswordCommand,
    }

    #[test]
    fn trim_newline_strips_one_lf_or_crlf() {
        assert_eq!(trim_newline("secret\n"), "secret");
        assert_eq!(trim_newline("secret\r\n"), "secret");
        assert_eq!(trim_newline("secret\n\n"), "secret\n");
        assert_eq!(trim_newline(" secret "), " secret ");
    }

    #[test]
    fn deserializes_from_string_array() {
        let parsed: Wrapper = toml::from_str(r#"cmd = ["pass", "show", "caldav"]"#).unwrap();
        assert_eq!(parsed.cmd, command(&["pass", "show", "caldav"]));
    }

    #[test]
    fn rejects_string() {
        assert!(toml::from_str::<Wrapper>(r#"cmd = "pass show caldav""#).is_err());
    }

    #[test]
    fn rejects_empty_array() {
        let err = toml::from_str::<Wrapper>("cmd = []").unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_returns_stdout_without_trailing_newline() {
        let password = command(&["printf", "s3cret\\n"]).run().await.unwrap();
        assert_eq!(password, "s3cret");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_errors_on_nonzero_exit() {
        let err = command(&["false"]).run().await.unwrap_err();
        assert!(matches!(err, PasswordCommandError::Failed { .. }));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_errors_on_empty_output() {
        let err = command(&["true"]).run().await.unwrap_err();
        assert!(matches!(err, PasswordCommandError::Empty { .. }));
    }

    #[tokio::test]
    async fn run_errors_on_missing_program() {
        let err = command(&["caldir-no-such-program"])
            .run()
            .await
            .unwrap_err();
        assert!(matches!(err, PasswordCommandError::Spawn { .. }));
        assert!(err.to_string().contains("caldir-no-such-program"));
    }
}
