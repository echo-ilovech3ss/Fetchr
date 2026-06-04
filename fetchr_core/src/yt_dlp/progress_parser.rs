use regex::Regex;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub percentage: f64,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
}

pub trait ProgressParser: Send + Sync {
    fn parse(&self, line: &str) -> Option<DownloadProgress>;
}

/// Primary parser using yt-dlp's modern --progress-template JSON log output
pub struct JsonProgressParser;

#[derive(Debug, Deserialize)]
struct YtDlpProgressJson {
    status: Option<String>,
    downloaded_bytes: Option<u64>,
    total_bytes: Option<u64>,
    total_bytes_estimate: Option<u64>,
    speed: Option<f64>,
    eta: Option<i64>,
}

impl ProgressParser for JsonProgressParser {
    fn parse(&self, line: &str) -> Option<DownloadProgress> {
        let prefix = "download-json:";
        if !line.starts_with(prefix) {
            return None;
        }

        let json_str = &line[prefix.len()..];
        let raw: YtDlpProgressJson = serde_json::from_str(json_str).ok()?;

        if raw.status.as_deref() == Some("finished") {
            return Some(DownloadProgress {
                percentage: 100.0,
                speed: Some("0 B/s".to_string()),
                eta: Some("00:00".to_string()),
                downloaded_bytes: raw.total_bytes.or(raw.downloaded_bytes),
                total_bytes: raw.total_bytes,
            });
        }

        let dl_bytes = raw.downloaded_bytes.unwrap_or(0);
        let tot_bytes = raw.total_bytes.or(raw.total_bytes_estimate).unwrap_or(0);

        let percentage = if tot_bytes > 0 {
            (dl_bytes as f64 / tot_bytes as f64) * 100.0
        } else {
            0.0
        };

        // Format speed: bytes/sec to e.g. "4.23 MB/s"
        let speed_str = raw.speed.map(|s| format_bytes_per_sec(s));

        // Format ETA: seconds to e.g. "01:24"
        let eta_str = raw.eta.map(|e| format_seconds(e));

        Some(DownloadProgress {
            percentage: percentage.clamp(0.0, 100.0),
            speed: speed_str,
            eta: eta_str,
            downloaded_bytes: raw.downloaded_bytes,
            total_bytes: raw.total_bytes.or(raw.total_bytes_estimate),
        })
    }
}

/// Fallback parser using regular expressions to parse traditional yt-dlp stdout
pub struct RegexProgressParser {
    re: Regex,
}

impl RegexProgressParser {
    pub fn new() -> Self {
        // Example line: [download]  23.4% of   10.23MiB at    2.34MiB/s ETA 00:04
        // Or:           [download] 100% of   10.23MiB in 00:04
        let re = Regex::new(
            r"\[download\]\s+([0-9.]+)%\s+of\s+([0-9.a-zA-Z\s]+)\s+at\s+([0-9.a-zA-Z/]+)\s+ETA\s+([0-9:]+)"
        ).unwrap();
        
        Self { re }
    }
}

impl ProgressParser for RegexProgressParser {
    fn parse(&self, line: &str) -> Option<DownloadProgress> {
        if let Some(caps) = self.re.captures(line) {
            let percentage: f64 = caps.get(1)?.as_str().parse().ok()?;
            let _total_size = caps.get(2)?.as_str().trim().to_string();
            let speed = caps.get(3)?.as_str().trim().to_string();
            let eta = caps.get(4)?.as_str().trim().to_string();

            return Some(DownloadProgress {
                percentage: percentage.clamp(0.0, 100.0),
                speed: Some(speed),
                eta: Some(eta),
                downloaded_bytes: None,
                total_bytes: None,
            });
        }
        
        // Handle 100% completed line: "[download] 100% of 12.34MiB in 00:05"
        if line.contains("[download]") && line.contains("100%") && line.contains("in") {
            return Some(DownloadProgress {
                percentage: 100.0,
                speed: Some("0 B/s".to_string()),
                eta: Some("00:00".to_string()),
                downloaded_bytes: None,
                total_bytes: None,
            });
        }

        None
    }
}

/// Master composite parser that tries JSON first, and falls back to Regex
pub struct CompositeProgressParser {
    json_parser: JsonProgressParser,
    regex_parser: RegexProgressParser,
}

impl CompositeProgressParser {
    pub fn new() -> Self {
        Self {
            json_parser: JsonProgressParser,
            regex_parser: RegexProgressParser::new(),
        }
    }
}

impl ProgressParser for CompositeProgressParser {
    fn parse(&self, line: &str) -> Option<DownloadProgress> {
        if line.contains("[VideoConvertor]") || line.contains("[Merger]") || line.contains("[ffmpeg]") {
            return Some(DownloadProgress {
                percentage: 100.0,
                speed: Some("Transcoding".to_string()),
                eta: Some("GPU active".to_string()),
                downloaded_bytes: None,
                total_bytes: None,
            });
        }
        if line.contains("[ExtractAudio]") {
            return Some(DownloadProgress {
                percentage: 100.0,
                speed: Some("Extracting".to_string()),
                eta: Some("Audio".to_string()),
                downloaded_bytes: None,
                total_bytes: None,
            });
        }

        self.json_parser.parse(line)
            .or_else(|| self.regex_parser.parse(line))
    }
}

// Helpers
fn format_bytes_per_sec(bytes_sec: f64) -> String {
    if bytes_sec < 1024.0 {
        format!("{:.0} B/s", bytes_sec)
    } else if bytes_sec < 1024.0 * 1024.0 {
        format!("{:.2} KB/s", bytes_sec / 1024.0)
    } else if bytes_sec < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} MB/s", bytes_sec / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB/s", bytes_sec / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_seconds(secs: i64) -> String {
    if secs < 0 {
        return "--:--".to_string();
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;

    if h > 0 {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex_parser() {
        let parser = RegexProgressParser::new();
        let line = "[download]  23.4% of   10.23MiB at    2.34MiB/s ETA 00:04";
        let progress = parser.parse(line).unwrap();
        assert_eq!(progress.percentage, 23.4);
        assert_eq!(progress.speed.as_deref(), Some("2.34MiB/s"));
        assert_eq!(progress.eta.as_deref(), Some("00:04"));
    }

    #[test]
    fn test_json_parser() {
        let parser = JsonProgressParser;
        let line = r#"download-json:{"status":"downloading","downloaded_bytes":5242880,"total_bytes":10485760,"speed":1048576,"eta":5}"#;
        let progress = parser.parse(line).unwrap();
        assert_eq!(progress.percentage, 50.0);
        assert_eq!(progress.speed.as_deref(), Some("1.00 MB/s"));
        assert_eq!(progress.eta.as_deref(), Some("00:05"));
    }
}
