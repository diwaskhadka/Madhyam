//! Downloads (ureq, rustls) and the one error type the UI explains to the user.

use crate::engine::EngineError;
use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

/// What went wrong, in the words the status line uses (MainWindow.Explain on Windows).
#[derive(Debug, Clone)]
pub enum AppError {
    Engine(EngineError),
    Network(String),
    Timeout,
    Other(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Engine(e) if e.code == "api" => {
                write!(f, "The server refused the request: {}", e.message)
            }
            AppError::Engine(e) if e.code == "parse" => {
                write!(f, "The server's answer could not be read: {}", e.message)
            }
            AppError::Engine(e) => write!(f, "{}", e.message),
            AppError::Network(m) => write!(f, "Network error: {m}"),
            AppError::Timeout => write!(f, "The server took too long to answer."),
            AppError::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<EngineError> for AppError {
    fn from(e: EngineError) -> Self {
        AppError::Engine(e)
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::TimedOut {
            AppError::Timeout
        } else {
            AppError::Other(e.to_string())
        }
    }
}

impl From<ureq::Error> for AppError {
    fn from(e: ureq::Error) -> Self {
        match e {
            ureq::Error::StatusCode(code) => AppError::Network(format!("HTTP {code}")),
            ureq::Error::Timeout(_) => AppError::Timeout,
            ureq::Error::Io(io) if io.kind() == std::io::ErrorKind::TimedOut => AppError::Timeout,
            other => AppError::Network(other.to_string()),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Engine(EngineError {
            code: "parse".into(),
            message: e.to_string(),
        })
    }
}

pub type Res<T> = Result<T, AppError>;

/// The browser-like user agent sent unless a playlist has its own.
pub fn default_user_agent() -> String {
    format!(
        "Mozilla/5.0 (X11; Linux x86_64) {}/{}",
        crate::brand::NAME,
        crate::brand::VERSION
    )
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .user_agent(default_user_agent())
        .max_redirects(10)
        .build()
        .into()
}

static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| agent(Duration::from_secs(180)));
static QUICK: LazyLock<ureq::Agent> = LazyLock::new(|| agent(Duration::from_secs(20)));

fn request(
    agent: &ureq::Agent,
    url: &str,
    ua: Option<&str>,
) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
    let mut r = agent.get(url);
    if let Some(ua) = ua.filter(|u| !u.trim().is_empty()) {
        r = r.header("User-Agent", ua);
    }
    r
}

/// GET a whole (small) body: JSON answers, images.
pub fn get_bytes(url: &str, ua: Option<&str>) -> Res<Vec<u8>> {
    let mut resp = request(&AGENT, url, ua).call()?;
    Ok(resp
        .body_mut()
        .with_config()
        .limit(256 * 1024 * 1024)
        .read_to_vec()?)
}

/// GET with a short timeout (images, ratings, connection tests).
pub fn get_quick(url: &str, ua: Option<&str>) -> Res<Vec<u8>> {
    let mut resp = request(&QUICK, url, ua).call()?;
    Ok(resp
        .body_mut()
        .with_config()
        .limit(32 * 1024 * 1024)
        .read_to_vec()?)
}

/// Status code only (Test connection on an M3U URL).
pub fn probe(url: &str, ua: Option<&str>) -> Res<u16> {
    let r = QUICK
        .get(url)
        .config()
        .http_status_as_error(false)
        .build()
        .header(
            "User-Agent",
            ua.filter(|u| !u.is_empty())
                .map(str::to_string)
                .unwrap_or_else(default_user_agent),
        )
        .call()?;
    Ok(r.status().as_u16())
}

/// GET a whole body, telling `received` the bytes read so far (every 256 KB and at the end).
pub fn get_bytes_counted(url: &str, ua: Option<&str>, received: &dyn Fn(u64)) -> Res<Vec<u8>> {
    let resp = request(&AGENT, url, ua).call()?;
    let mut input = Counted::new(resp.into_body().into_reader(), received);
    let mut out = Vec::new();
    input.read_to_end(&mut out)?;
    Ok(out)
}

/// A reader that reports how much has been read through it.
struct Counted<'a, R: Read> {
    inner: R,
    received: &'a dyn Fn(u64),
    total: u64,
    reported: u64,
}

impl<'a, R: Read> Counted<'a, R> {
    fn new(inner: R, received: &'a dyn Fn(u64)) -> Self {
        Counted { inner, received, total: 0, reported: 0 }
    }
}

impl<R: Read> Read for Counted<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.total += n as u64;
        if n == 0 || self.total - self.reported >= 256 * 1024 {
            self.reported = self.total;
            (self.received)(self.total);
        }
        Ok(n)
    }
}

/// Stream a URL (or local file) to a temp file, gunzipping .gz content (guides often are).
pub fn download(location: &str, name: &str, ua: Option<&str>) -> Res<PathBuf> {
    download_counted(location, name, ua, &|_| {})
}

/// `download`, telling `received` the bytes read so far.
pub fn download_counted(location: &str, name: &str, ua: Option<&str>, received: &dyn Fn(u64)) -> Res<PathBuf> {
    let tmp = crate::paths::temp_file(name);
    let result = (|| -> Res<()> {
        let input: Box<dyn Read + '_> = if Path::new(location).is_file() {
            Box::new(Counted::new(std::fs::File::open(location)?, received))
        } else if let Some(path) = location.strip_prefix("file://") {
            Box::new(Counted::new(std::fs::File::open(path)?, received))
        } else {
            let resp = request(&AGENT, location, ua).call()?;
            Box::new(Counted::new(resp.into_body().into_reader(), received))
        };
        let mut buffered = BufReader::with_capacity(64 * 1024, input);
        let gz = matches!(buffered.fill_buf()?, [0x1f, 0x8b, ..]);
        let mut out = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        if gz {
            std::io::copy(&mut flate2::read::MultiGzDecoder::new(buffered), &mut out)?;
        } else {
            std::io::copy(&mut buffered, &mut out)?;
        }
        out.flush()?;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(tmp),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// POST JSON, used by the OpenSubtitles client. Returns (status, body).
pub fn post_json(
    url: &str,
    headers: &[(&str, String)],
    body: &serde_json::Value,
) -> Res<(u16, Vec<u8>)> {
    let mut r = QUICK.post(url).config().http_status_as_error(false).build();
    for (k, v) in headers {
        r = r.header(*k, v);
    }
    let mut resp = r
        .header("Content-Type", "application/json")
        .send(body.to_string())?;
    let status = resp.status().as_u16();
    Ok((status, resp.body_mut().read_to_vec()?))
}

/// GET with headers and no status-as-error, for APIs that explain errors in JSON.
pub fn get_with(url: &str, headers: &[(&str, String)]) -> Res<(u16, Vec<u8>)> {
    let mut r = QUICK.get(url).config().http_status_as_error(false).build();
    for (k, v) in headers {
        r = r.header(*k, v);
    }
    let mut resp = r.call()?;
    let status = resp.status().as_u16();
    Ok((status, resp.body_mut().read_to_vec()?))
}
