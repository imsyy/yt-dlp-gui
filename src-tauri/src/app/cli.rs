use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliOpenRequest {
    pub url: Option<String>,
    pub cookie_file: Option<String>,
    pub download_dir: Option<String>,
}

impl CliOpenRequest {
    pub fn is_empty(&self) -> bool {
        self.url.is_none() && self.cookie_file.is_none() && self.download_dir.is_none()
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct CliOptions {
    pub request: CliOpenRequest,
    pub ytdlp_path: Option<PathBuf>,
    pub deno_path: Option<PathBuf>,
}

pub fn parse_cli_args<I, S>(args: I) -> CliOptions
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut options = CliOptions::default();
    let mut args = args.into_iter().map(Into::into).peekable();

    while let Some(argument) = args.next() {
        let (flag, inline_value) = argument
            .split_once('=')
            .map_or((argument.as_str(), None), |(flag, value)| {
                (flag, Some(value.to_string()))
            });

        let mut value = || inline_value.clone().or_else(|| args.next());
        match flag {
            "--url" => options.request.url = value().filter(|item| !item.is_empty()),
            "--cookies" => options.request.cookie_file = value().filter(|item| !item.is_empty()),
            "--dir" => options.request.download_dir = value().filter(|item| !item.is_empty()),
            "--yt-dlp-path" => {
                options.ytdlp_path = value().filter(|item| !item.is_empty()).map(PathBuf::from)
            }
            "--deno-path" => {
                options.deno_path = value().filter(|item| !item.is_empty()).map(PathBuf::from)
            }
            _ if options.request.url.is_none()
                && (argument.starts_with("https://") || argument.starts_with("http://")) =>
            {
                options.request.url = Some(argument)
            }
            _ => {}
        }
    }

    options
}

#[cfg(test)]
mod tests {
    use super::{parse_cli_args, CliOpenRequest};
    use std::path::PathBuf;

    #[test]
    fn parses_automation_flags_in_both_supported_forms() {
        let parsed = parse_cli_args([
            "ydl-gui",
            "--url=https://example.com/video",
            "--cookies",
            "/tmp/cookies.txt",
            "--dir=/tmp/downloads",
            "--yt-dlp-path",
            "/opt/tools/yt-dlp",
            "--deno-path=/opt/tools/deno",
        ]);

        assert_eq!(
            parsed.request,
            CliOpenRequest {
                url: Some("https://example.com/video".to_string()),
                cookie_file: Some("/tmp/cookies.txt".to_string()),
                download_dir: Some("/tmp/downloads".to_string()),
            }
        );
        assert_eq!(parsed.ytdlp_path, Some(PathBuf::from("/opt/tools/yt-dlp")));
        assert_eq!(parsed.deno_path, Some(PathBuf::from("/opt/tools/deno")));
    }

    #[test]
    fn accepts_a_positional_http_url() {
        let parsed = parse_cli_args(["ydl-gui", "https://example.com/video"]);
        assert_eq!(
            parsed.request.url.as_deref(),
            Some("https://example.com/video")
        );
    }
}
