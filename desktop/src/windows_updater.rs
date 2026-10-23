//! Explicit signed Windows updates; user content and credentials are never inspected.
use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::StreamExt;
use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};

const FEED: &str = "https://updates.vaovao.app/windows/stable.json";
const DOWNLOAD_PREFIX: &str = "https://updates.vaovao.app/downloads/windows/";
const PUBLIC_KEY: &str = include_str!("windows-updater-public-key.txt");
const MAX_INSTALLER: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub schema: u8,
    pub version: String,
    pub build: String,
    pub notes: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub minimum_windows: String,
    pub architecture: String,
    pub bundle: String,
    pub installer: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    signed: String,
    signature: String,
}
#[derive(Clone, Default, Serialize)]
pub struct Status {
    pub phase: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub downloaded: u64,
    pub total: u64,
    pub error: Option<String>,
}
#[derive(Default)]
struct Data {
    status: Status,
    release: Option<Release>,
    staged: Option<tempfile::TempDir>,
}
#[derive(Default)]
pub struct Updates {
    data: Mutex<Data>,
    busy: AtomicBool,
    pub installing: AtomicBool,
}
struct Operation<'a>(&'a Updates);
impl Drop for Operation<'_> {
    fn drop(&mut self) {
        self.0.installing.store(false, Ordering::SeqCst);
        self.0.busy.store(false, Ordering::SeqCst);
    }
}
impl Updates {
    fn claim(&self) -> Result<Operation<'_>, String> {
        self.busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "更新操作正在进行 / Update already in progress".to_string())?;
        Ok(Operation(self))
    }
    pub fn status(&self) -> Status {
        self.data.lock().unwrap().status.clone()
    }
    fn fail(&self, error: &str) {
        let mut d = self.data.lock().unwrap();
        d.status.phase = "failed".into();
        d.status.error = Some(error.to_owned());
    }
    pub async fn check(&self) -> Result<Status, String> {
        if !cfg!(target_os = "windows") {
            return Err("Windows updater unavailable on this platform".into());
        }
        let _guard = self.claim()?;
        {
            let mut d = self.data.lock().unwrap();
            *d = Data::default();
            d.status.phase = "checking".into();
        }
        let result = check_feed(&Source::production(), current_version(), current_build()).await;
        match result {
            Ok(release) => {
                let mut d = self.data.lock().unwrap();
                d.status.phase = if release.is_some() {
                    "available"
                } else {
                    "current"
                }
                .into();
                if let Some(r) = &release {
                    d.status.version = Some(r.version.clone());
                    d.status.notes = Some(r.notes.clone());
                    d.status.total = r.size;
                }
                d.release = release;
                Ok(d.status.clone())
            }
            Err(e) => {
                self.fail(&e);
                Err(e)
            }
        }
    }
    pub async fn download(&self) -> Result<Status, String> {
        let _guard = self.claim()?;
        let release = self
            .data
            .lock()
            .unwrap()
            .release
            .clone()
            .ok_or("请先检查更新 / Check for updates first")?;
        {
            let mut d = self.data.lock().unwrap();
            d.staged = None;
            d.status.phase = "downloading".into();
            d.status.error = None;
            d.status.downloaded = 0;
        }
        let result = async {
            installed_directory()?;
            let bytes = download(&Source::production(), &release, |n| {
                self.data.lock().unwrap().status.downloaded = n
            })
            .await?;
            tauri::async_runtime::spawn_blocking(move || stage(&bytes, &release))
                .await
                .map_err(|_| "更新准备中断 / Update preparation interrupted".to_string())?
        }
        .await;
        match result {
            Ok(staged) => {
                let mut d = self.data.lock().unwrap();
                d.staged = Some(staged);
                d.status.phase = "ready".into();
                Ok(d.status.clone())
            }
            Err(e) => {
                self.fail(&e);
                Err(e)
            }
        }
    }
    pub fn install(&self, idle: impl FnOnce() -> bool) -> Result<(), String> {
        self.install_with(idle, |path| launch_installer(path, &installed_directory()?))
    }
    fn install_with(
        &self,
        idle: impl FnOnce() -> bool,
        launch: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        let guard = self.claim()?;
        self.installing.store(true, Ordering::SeqCst);
        // The caller checks this while holding the same runtime mutex as session.begin().
        if !idle() {
            return Err(
                "请先结束语音输入并处理待回填结果 / Finish voice input and pending insertion first"
                    .into(),
            );
        }
        let mut d = self.data.lock().unwrap();
        let release = d
            .release
            .as_ref()
            .ok_or("请先下载更新 / Download the update first")?;
        let staged = d
            .staged
            .as_ref()
            .ok_or("请先下载更新 / Download the update first")?;
        let path = staged.path().join("Vao-update.exe");
        let result = (|| {
            let mut file = locked_installer(&path)?;
            let mut bytes = Vec::new();
            (&mut file)
                .take(MAX_INSTALLER + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "Cannot read staged update")?;
            verify_installer(&bytes, release)?;
            // Keep the verified executable open without write/delete sharing through CreateProcess.
            launch(&path)
        })();
        match result {
            Ok(()) => {
                // NSIS may still be reading its original file after this application exits.
                let _ = d.staged.take().unwrap().keep();
                d.status.phase = "installing".into();
                d.status.error = None;
                std::mem::forget(guard);
                Ok(())
            }
            Err(e) => {
                d.status.phase = "failed".into();
                d.status.error = Some(e.clone());
                Err(e)
            }
        }
    }
}
fn metadata() -> serde_json::Value {
    serde_json::from_str(include_str!("../../release.windows.json"))
        .expect("Windows release metadata")
}
fn current_version() -> String {
    metadata()["version"].as_str().unwrap().to_owned()
}
fn current_build() -> String {
    metadata()["build"].as_str().unwrap().to_owned()
}
fn trusted_origin(url: &url::Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some("updates.vaovao.app")
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}
struct Source {
    endpoint: String,
    fixture_origin: Option<String>,
}
impl Source {
    fn production() -> Self {
        Self {
            endpoint: FEED.into(),
            fixture_origin: None,
        }
    }
    fn allows(&self, url: &url::Url) -> bool {
        if let Some(origin) = &self.fixture_origin {
            return cfg!(test)
                && url.origin().ascii_serialization() == *origin
                && url.host_str() == Some("127.0.0.1")
                && url.username().is_empty()
                && url.password().is_none();
        }
        trusted_origin(url)
    }
    fn client(&self) -> Result<reqwest::Client, String> {
        let fixture_origin = self.fixture_origin.clone();
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(12))
            .timeout(Duration::from_secs(180))
            .user_agent("Vao-Windows-Updater")
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                let target = attempt.url();
                let allowed = if let Some(origin) = &fixture_origin {
                    cfg!(test)
                        && target.origin().ascii_serialization() == *origin
                        && target.host_str() == Some("127.0.0.1")
                        && target.username().is_empty()
                        && target.password().is_none()
                } else {
                    trusted_origin(target)
                };
                if attempt.previous().len() >= 5 || !allowed {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .map_err(|_| "无法建立更新连接 / Could not create update connection".into())
    }
}
fn decoded_release(bytes: &[u8], key: &str) -> Result<Release, String> {
    if bytes.len() > 32768 {
        return Err("更新清单过大 / Update manifest too large".into());
    }
    let envelope: Envelope =
        serde_json::from_slice(bytes).map_err(|_| "更新清单格式无效 / Invalid update manifest")?;
    let signed = STANDARD
        .decode(envelope.signed)
        .map_err(|_| "Invalid signed manifest")?;
    let sig_text = String::from_utf8(
        STANDARD
            .decode(envelope.signature)
            .map_err(|_| "Invalid update signature")?,
    )
    .map_err(|_| "Invalid update signature")?;
    let signature =
        Signature::decode(&sig_text).map_err(|_| "更新签名无效 / Invalid update signature")?;
    PublicKey::from_base64(key.trim())
        .map_err(|_| "Invalid embedded update key")?
        .verify(&signed, &signature, false)
        .map_err(|_| "更新验签失败，已停止 / Update signature verification failed")?;
    let release: Release = serde_json::from_slice(&signed)
        .map_err(|_| "更新平台或字段无效 / Invalid Windows release fields")?;
    validate_release(&release)?;
    Ok(release)
}
fn validate_release(r: &Release) -> Result<(), String> {
    let version = semver::Version::parse(&r.version).map_err(|_| "Invalid release version")?;
    if r.schema != 1
        || r.bundle != "app.vaovao.windows"
        || r.architecture != "x86_64"
        || r.installer != "nsis"
        || r.minimum_windows != "10.0.17763"
        || !(1..=MAX_INSTALLER).contains(&r.size)
        || r.sha256.len() != 64
        || !r.sha256.bytes().all(|c| c.is_ascii_hexdigit())
        || r.notes.len() > 8000
        || r.version.len() > 100
        || r.build.len() > 40
        || !version.pre.is_empty()
        || !version.build.is_empty()
    {
        return Err("更新不适用于此应用 / Update is incompatible with this app".into());
    }
    build_number(&r.build)?;
    let url = url::Url::parse(&r.url).map_err(|_| "Invalid update URL")?;
    if r.url.len() > 2048
        || !trusted_origin(&url)
        || !r.url.starts_with(DOWNLOAD_PREFIX)
        || !url.path().starts_with("/downloads/windows/")
        || !url.path().ends_with(".exe")
        || url.path().contains('%')
        || url.path().contains('\\')
    {
        return Err("更新下载来源无效 / Invalid update download source".into());
    }
    Ok(())
}
fn build_number(build: &str) -> Result<Vec<u64>, String> {
    if build
        .split('.')
        .any(|s| s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err("Invalid build number".into());
    }
    let parts: Vec<u64> = build
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| "Invalid build number")?;
    if parts.len() != 2 {
        return Err("Invalid build number".into());
    }
    Ok(parts)
}
fn newer(r: &Release, version: String, build: String) -> Result<bool, String> {
    let offered = semver::Version::parse(&r.version).map_err(|_| "Invalid update version")?;
    let installed = semver::Version::parse(&version).map_err(|_| "Invalid current version")?;
    Ok(offered > installed
        || (offered == installed && build_number(&r.build)? > build_number(&build)?))
}
async fn response_bytes(
    response: reqwest::Response,
    limit: u64,
    progress: impl Fn(u64),
) -> Result<Vec<u8>, String> {
    if !response.status().is_success() {
        return Err(format!(
            "更新服务暂不可用 / Update service unavailable (HTTP {})",
            response.status().as_u16()
        ));
    }
    if response.content_length().is_some_and(|n| n > limit) {
        return Err("更新文件超过大小限制 / Update exceeds size limit".into());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "下载中断，请重试 / Download interrupted; please retry")?;
        if bytes.len() as u64 + chunk.len() as u64 > limit {
            return Err("更新文件超过大小限制 / Update exceeds size limit".into());
        }
        bytes.extend_from_slice(&chunk);
        progress(bytes.len() as u64);
    }
    Ok(bytes)
}
async fn check_feed(
    source: &Source,
    version: String,
    build: String,
) -> Result<Option<Release>, String> {
    let endpoint = url::Url::parse(&source.endpoint).map_err(|_| "Invalid update endpoint")?;
    if !source.allows(&endpoint) {
        return Err("Untrusted update endpoint".into());
    }
    let response = source
        .client()?
        .get(endpoint)
        .send()
        .await
        .map_err(|_| "无法连接更新服务，请稍后重试 / Could not reach update service")?;
    if response.status().as_u16() == 404 {
        return Err("尚未发布 Windows 更新清单，请稍后重试 / No Windows update manifest has been published yet".into());
    }
    let release = decoded_release(&response_bytes(response, 32768, |_| {}).await?, PUBLIC_KEY)?;
    Ok(if newer(&release, version, build)? {
        Some(release)
    } else {
        None
    })
}
async fn download(
    source: &Source,
    release: &Release,
    progress: impl Fn(u64),
) -> Result<Vec<u8>, String> {
    let url = url::Url::parse(&release.url).map_err(|_| "Invalid update URL")?;
    if !source.allows(&url) {
        return Err("Untrusted update download".into());
    }
    let response = source
        .client()?
        .get(url)
        .send()
        .await
        .map_err(|_| "无法下载更新 / Could not download update")?;
    let bytes = response_bytes(response, release.size, progress).await?;
    verify_installer(&bytes, release)?;
    Ok(bytes)
}
fn verify_installer(bytes: &[u8], release: &Release) -> Result<(), String> {
    if bytes.len() as u64 != release.size
        || bytes.len() as u64 > MAX_INSTALLER
        || format!("{:x}", Sha256::digest(bytes)) != release.sha256.to_lowercase()
    {
        return Err("更新包校验失败，已停止 / Update installer verification failed".into());
    }
    let offset = bytes
        .get(60..64)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()) as usize)
        .ok_or("Invalid installer header")?;
    let header = bytes
        .get(offset..offset.saturating_add(6))
        .ok_or("Invalid installer header")?;
    if !bytes.starts_with(b"MZ")
        || &header[..4] != b"PE\0\0"
        || !matches!(u16::from_le_bytes([header[4], header[5]]), 0x14c | 0x8664)
    {
        return Err("更新包不是受支持的 Windows 安装程序 / Unsupported Windows installer".into());
    }
    Ok(())
}
fn stage(bytes: &[u8], release: &Release) -> Result<tempfile::TempDir, String> {
    verify_installer(bytes, release)?;
    let dir = tempfile::Builder::new()
        .prefix("Vao-update-")
        .tempdir()
        .map_err(|_| "无法准备更新目录 / Cannot prepare update directory")?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.path().join("Vao-update.exe"))
        .map_err(|_| "Cannot stage update")?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Cannot write staged update")?;
    Ok(dir)
}
fn locked_installer(path: &Path) -> Result<std::fs::File, String> {
    if path.is_symlink() || !path.is_file() {
        return Err("Invalid staged installer".into());
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    options
        .open(path)
        .map_err(|_| "更新文件已变化或被占用 / Update file changed or unavailable".into())
}
#[cfg(target_os = "windows")]
fn installed_directory() -> Result<PathBuf, String> {
    use winreg::{
        enums::{HKEY_CURRENT_USER, KEY_READ},
        RegKey,
    };
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Vao", KEY_READ)
        .map_err(|_| "请使用已安装的 Vao 更新；便携版请手动安装 / Use the installed Vao; portable copies require manual installation")?;
    let location: String = key
        .get_value("InstallLocation")
        .map_err(|_| "Cannot locate installed Vao")?;
    let path = PathBuf::from(location.trim_matches('"'));
    let executable = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|_| "Cannot resolve running Vao")?;
    if path.is_symlink()
        || !path.join("uninstall.exe").is_file()
        || path
            .join("vao.exe")
            .canonicalize()
            .map_err(|_| "Cannot resolve installed Vao")?
            != executable
    {
        return Err(
            "请从当前用户已安装的 Vao 更新 / Run the Vao installed for the current user".into(),
        );
    }
    path.canonicalize()
        .map_err(|_| "Cannot resolve installation directory".into())
}
#[cfg(not(target_os = "windows"))]
fn installed_directory() -> Result<PathBuf, String> {
    Err("Windows updater unavailable on this platform".into())
}
#[cfg(target_os = "windows")]
fn launch_installer(path: &Path, destination: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    // No shell, elevation, credential prompt automation, uninstall or data-directory arguments.
    let destination = destination
        .to_str()
        .ok_or("Unsupported installation path")?;
    let destination = destination.strip_prefix("\\\\?\\").unwrap_or(destination);
    if destination.contains(['"', '\r', '\n', '\0']) {
        return Err("Unsupported installation path".into());
    }
    // NSIS /D= must be the last argument and consumes its unquoted remainder,
    // including spaces. Do not use a shell or Command's ordinary quoting for it.
    std::process::Command::new(path)
        .args(["/UPDATE", "/P", "/R"])
        .raw_arg(format!("/D={destination}"))
        .spawn()
        .map_err(|_| {
            "无法启动安装程序，原版本保持 / Could not start installer; current version retained"
        })?;
    Ok(())
}
#[cfg(not(target_os = "windows"))]
fn launch_installer(_: &Path, _: &Path) -> Result<(), String> {
    Err("Windows installer unavailable on this platform".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &[u8] = include_bytes!("fixtures/windows-updater-valid.json");
    const FIXTURE_KEY: &str = include_str!("fixtures/windows-updater-public.txt");
    fn release() -> Release {
        decoded_release(FIXTURE, FIXTURE_KEY).unwrap()
    }
    fn installer() -> Vec<u8> {
        let mut bytes = vec![0; 512];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&128u32.to_le_bytes());
        bytes[128..132].copy_from_slice(b"PE\0\0");
        bytes[132..134].copy_from_slice(&0x14cu16.to_le_bytes());
        bytes
    }
    fn ready() -> Updates {
        let updates = Updates::default();
        {
            let r = release();
            let staged = stage(&installer(), &r).unwrap();
            let mut d = updates.data.lock().unwrap();
            d.release = Some(r);
            d.staged = Some(staged);
            d.status.phase = "ready".into();
        }
        updates
    }
    fn serve(status: &str, headers: &str, bytes: &[u8]) -> (Source, std::thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let mut response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n",
            bytes.len()
        )
        .into_bytes();
        response.extend_from_slice(bytes);
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request).unwrap();
            stream.write_all(&response).unwrap();
        });
        (
            Source {
                endpoint: format!("{origin}/feed"),
                fixture_origin: Some(origin),
            },
            thread,
        )
    }
    #[test]
    fn valid_signature_and_production_key_are_independent() {
        assert_eq!(release().version, "0.2.3");
        assert!(decoded_release(FIXTURE, PUBLIC_KEY).is_err());
        assert!(decoded_release(&vec![0; 32769], FIXTURE_KEY).is_err());
    }
    #[test]
    fn signature_covers_every_installer_and_platform_field() {
        let original: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
        let payload: serde_json::Value = serde_json::from_slice(
            &STANDARD
                .decode(original["signed"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
        for field in [
            "version",
            "build",
            "notes",
            "url",
            "sha256",
            "size",
            "bundle",
            "architecture",
            "minimum_windows",
            "installer",
        ] {
            let mut envelope = original.clone();
            let mut altered = payload.clone();
            altered[field] = "tampered".into();
            envelope["signed"] = STANDARD
                .encode(serde_json::to_vec(&altered).unwrap())
                .into();
            assert!(
                decoded_release(&serde_json::to_vec(&envelope).unwrap(), FIXTURE_KEY).is_err(),
                "{field}"
            );
        }
    }
    #[test]
    fn windows_identity_stable_channel_and_limits_are_required() {
        for (field, value) in [
            ("architecture", "arm64"),
            ("bundle", "app.vaovao.desktop"),
            ("installer", "msi"),
            ("minimum_windows", "6.1"),
            ("version", "0.2.4-preview.1"),
            ("version", "0.2.3+unreviewed"),
            ("build", "+1.2"),
            ("build", "1.2.3"),
            ("sha256", "bad"),
        ] {
            let mut value_json = serde_json::to_value(release()).unwrap();
            value_json[field] = value.into();
            assert!(
                validate_release(&serde_json::from_value(value_json).unwrap()).is_err(),
                "{field}"
            );
        }
        let mut r = release();
        r.size = 0;
        assert!(validate_release(&r).is_err());
        r.size = MAX_INSTALLER + 1;
        assert!(validate_release(&r).is_err());
        r = release();
        r.notes = "x".repeat(8001);
        assert!(validate_release(&r).is_err());
    }
    #[test]
    fn official_origin_and_windows_download_path_cannot_escape() {
        for url in [
            "http://updates.vaovao.app/downloads/windows/a.exe",
            "https://updates.vaovao.app:444/downloads/windows/a.exe",
            "https://user@updates.vaovao.app/downloads/windows/a.exe",
            "https://updates.vaovao.app.evil.test/downloads/windows/a.exe",
            "https://github.com/loewe1631/Vao/releases/download/a.exe",
            "https://updates.vaovao.app/downloads/mac.exe",
            "https://updates.vaovao.app/downloads/windows/../mac.exe",
            "https://updates.vaovao.app/downloads/windows/a.exe?token=x",
            "https://updates.vaovao.app/downloads/windows/a.exe#x",
            "https://updates.vaovao.app/downloads/windows/a%2F.exe",
        ] {
            let mut r = release();
            r.url = url.into();
            assert!(validate_release(&r).is_err(), "{url}");
        }
        assert!(Source::production().allows(&url::Url::parse(FEED).unwrap()));
        assert!(
            !Source::production().allows(&url::Url::parse("http://127.0.0.1:1234/feed").unwrap())
        );
    }
    #[test]
    fn version_and_build_comparison_prevents_downgrades() {
        let r = release();
        assert!(newer(&r, "0.2.3-intent.preview.2".into(), "20261023.9999".into()).unwrap());
        assert!(!newer(&r, "0.2.4".into(), "20261023.1".into()).unwrap());
        assert!(newer(&r, "0.2.3".into(), "20261023.3015".into()).unwrap());
        assert!(!newer(&r, "0.2.3".into(), "20261023.3016".into()).unwrap());
        assert!(!newer(&r, "0.2.3".into(), "20261024.1".into()).unwrap());
    }
    #[test]
    fn tampered_truncated_and_non_pe_installers_are_rejected() {
        let bytes = installer();
        let r = release();
        assert!(verify_installer(&bytes, &r).is_ok());
        let mut altered = bytes.clone();
        altered[200] ^= 1;
        assert!(stage(&altered, &r).is_err());
        assert!(stage(&bytes[..511], &r).is_err());
        for offset in [0, 60, 128, 133] {
            let mut altered = bytes.clone();
            altered[offset] ^= 0xff;
            let mut signed = r.clone();
            signed.sha256 = format!("{:x}", Sha256::digest(&altered));
            assert!(stage(&altered, &signed).is_err());
        }
    }
    #[test]
    fn staging_cleans_up_when_rechecking_or_abandoning_download() {
        let updates = ready();
        let path = updates
            .data
            .lock()
            .unwrap()
            .staged
            .as_ref()
            .unwrap()
            .path()
            .to_owned();
        assert!(path.join("Vao-update.exe").is_file());
        *updates.data.lock().unwrap() = Data::default();
        assert!(!path.exists());
    }
    #[test]
    fn install_requires_idle_and_releases_gate_after_rejection() {
        let updates = ready();
        assert!(updates
            .install_with(
                || false,
                |_| panic!("active voice must not launch installer")
            )
            .is_err());
        assert!(!updates.installing.load(Ordering::SeqCst));
        assert_eq!(updates.status().phase, "ready");
        assert!(updates.claim().is_ok());
    }
    #[test]
    fn changed_download_is_reverified_before_any_process_launch() {
        let updates = ready();
        let path = updates
            .data
            .lock()
            .unwrap()
            .staged
            .as_ref()
            .unwrap()
            .path()
            .join("Vao-update.exe");
        std::fs::write(path, b"changed after download").unwrap();
        assert!(updates
            .install_with(|| true, |_| panic!("tampered installer must not execute"))
            .is_err());
        assert!(!updates.installing.load(Ordering::SeqCst));
        assert_eq!(updates.status().phase, "failed");
    }
    #[test]
    fn launch_failure_retains_download_and_does_not_lock_voice() {
        let updates = ready();
        assert!(updates
            .install_with(|| true, |_| Err("synthetic launch failure".into()))
            .is_err());
        assert!(updates
            .data
            .lock()
            .unwrap()
            .staged
            .as_ref()
            .unwrap()
            .path()
            .join("Vao-update.exe")
            .exists());
        assert!(!updates.installing.load(Ordering::SeqCst));
        assert!(updates.claim().is_ok());
    }
    #[cfg(target_os = "windows")]
    #[test]
    fn verified_installer_cannot_be_rewritten_or_deleted_while_launching() {
        let staged = stage(&installer(), &release()).unwrap();
        let path = staged.path().join("Vao-update.exe");
        let file = locked_installer(&path).unwrap();
        assert!(std::fs::OpenOptions::new().write(true).open(&path).is_err());
        assert!(std::fs::remove_file(&path).is_err());
        drop(file);
        assert!(std::fs::remove_file(path).is_ok());
    }
    #[test]
    fn only_successful_handoff_keeps_install_gate_closed() {
        let updates = ready();
        let path = updates
            .data
            .lock()
            .unwrap()
            .staged
            .as_ref()
            .unwrap()
            .path()
            .to_owned();
        let guard = updates.claim().unwrap();
        assert!(updates.claim().is_err());
        drop(guard);
        updates
            .install_with(
                || true,
                |p| {
                    assert_eq!(p.file_name().unwrap(), "Vao-update.exe");
                    Ok(())
                },
            )
            .unwrap();
        assert!(updates.installing.load(Ordering::SeqCst));
        assert!(updates.claim().is_err());
        assert_eq!(updates.status().phase, "installing");
        assert!(path.is_dir());
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn missing_feed_is_an_error_instead_of_up_to_date() {
        let (source, thread) = serve("404 Not Found", "", b"missing");
        assert!(check_feed(&source, "0.2.2".into(), "20261022.3014".into())
            .await
            .unwrap_err()
            .contains("尚未发布"));
        thread.join().unwrap();
    }
    #[tokio::test]
    async fn wrong_signature_from_feed_never_offers_an_update() {
        let (source, thread) = serve("200 OK", "", FIXTURE);
        assert!(check_feed(&source, "0.2.2".into(), "20261022.3014".into())
            .await
            .is_err());
        thread.join().unwrap();
    }
    #[tokio::test]
    async fn streamed_download_checks_size_hash_and_redirect_boundaries() {
        let bytes = installer();
        let (source, thread) = serve("200 OK", "", &bytes);
        let mut r = release();
        r.url = format!("{}/installer.exe", source.fixture_origin.as_ref().unwrap());
        assert_eq!(download(&source, &r, |_| {}).await.unwrap(), bytes);
        thread.join().unwrap();
        let (source, thread) = serve("200 OK", "", &vec![0; 513]);
        r.url = format!("{}/installer.exe", source.fixture_origin.as_ref().unwrap());
        assert!(download(&source, &r, |_| {}).await.is_err());
        thread.join().unwrap();
        let (source, thread) = serve(
            "302 Found",
            "Location: https://example.invalid/installer.exe\r\n",
            b"",
        );
        r.url = format!("{}/installer.exe", source.fixture_origin.as_ref().unwrap());
        assert!(download(&source, &r, |_| {})
            .await
            .unwrap_err()
            .contains("HTTP 302"));
        thread.join().unwrap();
    }
}
