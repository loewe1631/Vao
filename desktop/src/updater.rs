//! Explicit, signed Mac updates. No background downloads or access to user content/credentials.
use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::StreamExt;
use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::{Component, Path, PathBuf}, sync::{atomic::{AtomicBool, Ordering}, Mutex}, time::Duration};

const FEED: &str = "https://updates.vaovao.app/macos/stable.json";
const DOWNLOAD_PREFIX: &str = "https://updates.vaovao.app/downloads/";
const TEAM: &str = "FP265645B8";
const BUNDLE: &str = "app.vaovao.desktop";
const MAX_ARCHIVE: u64 = 128 * 1024 * 1024;
const MAX_EXTRACTED: u64 = 512 * 1024 * 1024;
const PUBLIC_KEY: &str = include_str!("updater-public-key.txt");

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub schema: u8,
    pub version: String,
    pub build: String,
    pub notes: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    pub minimum_macos: String,
    pub architecture: String,
    pub bundle: String,
    pub team: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope { signed: String, signature: String }
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
struct Data { status: Status, release: Option<Release>, staged: Option<tempfile::TempDir> }
#[derive(Default)]
pub struct Updates { data: Mutex<Data>, busy: AtomicBool, pub installing: AtomicBool }
struct Operation<'a>(&'a Updates);
impl Drop for Operation<'_> {
    fn drop(&mut self) { self.0.installing.store(false, Ordering::SeqCst); self.0.busy.store(false, Ordering::SeqCst); }
}
impl Updates {
    fn claim(&self) -> Result<Operation<'_>, String> {
        self.busy.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "更新操作正在进行 / Update already in progress".to_string())?;
        Ok(Operation(self))
    }
    pub fn status(&self) -> Status { self.data.lock().unwrap().status.clone() }
    fn fail(&self, error: &str) {
        let mut d = self.data.lock().unwrap();
        d.status.phase = "failed".into(); d.status.error = Some(error.to_string());
    }
    pub async fn check(&self) -> Result<Status, String> {
        if !cfg!(target_os = "macos") {return Err("Software updates are currently available on macOS only".into());}
        let _guard = self.claim()?;
        { let mut d = self.data.lock().unwrap();
          *d = Data::default(); d.status.phase = "checking".into(); }
        let result = check_feed(&Source::production(), current_version(), current_build()).await;
        match result {
            Ok(release) => {
                let mut d = self.data.lock().unwrap();
                d.status.phase = if release.is_some() { "available" } else { "current" }.into();
                if let Some(r) = &release { d.status.version = Some(r.version.clone()); d.status.notes = Some(r.notes.clone()); d.status.total = r.size; }
                d.release = release; Ok(d.status.clone())
            }
            Err(e) => { self.fail(&e); Err(e) }
        }
    }
    pub async fn download(&self) -> Result<Status, String> {
        let _guard = self.claim()?;
        let release = self.data.lock().unwrap().release.clone().ok_or("请先检查更新 / Check for updates first")?;
        { let mut d = self.data.lock().unwrap(); d.staged = None; d.status.phase = "downloading".into(); d.status.error = None; d.status.downloaded = 0; }
        let result = async {
            let target = installed_app()?;
            validate_destination(&target)?;
            let bytes = download(&Source::production(), &release, |n| self.data.lock().unwrap().status.downloaded = n).await?;
            tauri::async_runtime::spawn_blocking(move || prepare(&bytes, &release, &target)).await
                .map_err(|_| "更新解包中断 / Update preparation interrupted".to_string())?
        }.await;
        match result {
            Ok(staged) => { let mut d = self.data.lock().unwrap(); d.staged = Some(staged); d.status.phase = "ready".into(); Ok(d.status.clone()) }
            Err(e) => { self.fail(&e); Err(e) }
        }
    }
    // Caller reserves this before checking recording state, preventing a new session during replacement.
    pub fn install(&self, idle: impl FnOnce() -> bool) -> Result<PathBuf, String> {
        let _guard = self.claim()?;
        self.installing.store(true, Ordering::SeqCst);
        if !idle() { return Err("请先结束语音输入，或复制待处理结果 / Finish voice input or copy the pending result first".into()); }
        let mut d = self.data.lock().unwrap();
        let release = d.release.clone().ok_or("请先下载更新 / Download the update first")?;
        let staged = d.staged.take().ok_or("请先下载更新 / Download the update first")?;
        d.status.phase = "installing".into(); d.status.error = None;
        let result = installed_app().and_then(|target| install_staged(staged, &release, &target));
        match result {
            Ok(backup) => {
                // Keep the installation gate closed until process exit and relaunch.
                d.status.phase = "installed".into(); std::mem::forget(_guard); Ok(backup)
            }
            Err(e) => { d.status.phase = "failed".into(); d.status.error = Some(e.clone()); Err(e) }
        }
    }
}
fn metadata() -> serde_json::Value { serde_json::from_str(include_str!("../../release.json")).expect("release metadata") }
fn current_version() -> String { metadata()["version"].as_str().unwrap().to_string() }
fn current_build() -> String { metadata()["build"].as_str().unwrap().to_string() }

struct Source { endpoint: String, fixture_origin: Option<String>, ca: Option<Vec<u8>> }
impl Source {
    fn production() -> Self { Self { endpoint: FEED.into(), fixture_origin: None, ca: None } }
    fn allows(&self, url: &url::Url) -> bool {
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() { return false; }
        if let Some(origin) = &self.fixture_origin { return url.origin().ascii_serialization() == *origin; }
        url.host_str().is_some_and(|h| h == "updates.vaovao.app")
    }
    fn client(&self) -> Result<reqwest::Client, String> {
        let allowed_origin = self.fixture_origin.clone();
        let mut builder = reqwest::Client::builder().connect_timeout(Duration::from_secs(12))
            .timeout(Duration::from_secs(180)).user_agent("Vao-Mac-Updater")
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                let target = attempt.url();
                let permitted = target.scheme() == "https" && target.username().is_empty() && target.password().is_none()
                    && if let Some(origin) = &allowed_origin {target.origin().ascii_serialization() == *origin}
                    else {target.host_str().is_some_and(|h| h == "updates.vaovao.app")};
                if attempt.previous().len() >= 5 || !permitted { attempt.stop() } else { attempt.follow() }
            }));
        if let Some(ca) = &self.ca { builder = builder.add_root_certificate(reqwest::Certificate::from_pem(ca).map_err(|_| "Invalid fixture certificate")?); }
        builder.build().map_err(|_| "无法建立更新连接 / Could not create update connection".into())
    }
}
fn decoded_release(bytes: &[u8]) -> Result<Release, String> {
    if bytes.len() > 32768 { return Err("更新清单过大 / Update manifest too large".into()); }
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| "更新清单格式无效 / Invalid update manifest")?;
    let signed = STANDARD.decode(envelope.signed).map_err(|_| "Invalid signed manifest")?;
    let sig_text = String::from_utf8(STANDARD.decode(envelope.signature).map_err(|_| "Invalid update signature")?).map_err(|_| "Invalid update signature")?;
    let signature = Signature::decode(&sig_text).map_err(|_| "更新签名无效 / Invalid update signature")?;
    PublicKey::from_base64(PUBLIC_KEY.trim()).map_err(|_| "Invalid embedded update key")?
        .verify(&signed, &signature, false).map_err(|_| "更新验签失败，已停止 / Update signature verification failed")?;
    let r: Release = serde_json::from_slice(&signed).map_err(|_| "Invalid signed release fields")?;
    if r.schema != 1 || r.bundle != BUNDLE || r.team != TEAM || r.architecture != "arm64"
        || !(1..=MAX_ARCHIVE).contains(&r.size) || r.sha256.len() != 64 || !r.sha256.bytes().all(|c| c.is_ascii_hexdigit())
        || r.notes.len() > 8000 || r.url.len() > 2048 || r.version.len() > 100 || r.build.len() > 40
        || r.minimum_macos != "12.0" {
        return Err("更新不适用于此应用 / Update is incompatible with this app".into());
    }
    semver::Version::parse(&r.version).map_err(|_| "Invalid release version")?;
    build_number(&r.build)?;
    Ok(r)
}
fn build_number(build: &str) -> Result<Vec<u64>, String> {
    let parts: Result<Vec<_>, _> = build.split('.').map(str::parse::<u64>).collect();
    let values = parts.map_err(|_| "Invalid build number")?;
    if values.len() != 2 { return Err("Invalid build number".into()); } Ok(values)
}
fn newer(r: &Release, version: String, build: String) -> Result<bool, String> {
    let offered = semver::Version::parse(&r.version).map_err(|_| "Invalid update version")?;
    let installed = semver::Version::parse(&version).map_err(|_| "Invalid current version")?;
    Ok(offered > installed || (offered == installed && build_number(&r.build)? > build_number(&build)?))
}
async fn response_bytes(response: reqwest::Response, limit: u64, progress: impl Fn(u64)) -> Result<Vec<u8>, String> {
    if !response.status().is_success() { return Err(format!("更新服务暂不可用 / Update service unavailable (HTTP {})", response.status().as_u16())); }
    if response.content_length().is_some_and(|n| n > limit) { return Err("更新文件超过大小限制 / Update exceeds size limit".into()); }
    let mut stream = response.bytes_stream(); let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "下载中断，请重试 / Download interrupted; please retry")?;
        if bytes.len() as u64 + chunk.len() as u64 > limit { return Err("更新文件超过大小限制 / Update exceeds size limit".into()); }
        bytes.extend_from_slice(&chunk); progress(bytes.len() as u64);
    }
    Ok(bytes)
}
async fn check_feed(source: &Source, version: String, build: String) -> Result<Option<Release>, String> {
    let endpoint = url::Url::parse(&source.endpoint).map_err(|_| "Invalid update endpoint")?;
    if !source.allows(&endpoint) { return Err("Untrusted update endpoint".into()); }
    let response = source.client()?.get(endpoint).send().await.map_err(|_| "无法连接更新服务，请稍后重试 / Could not reach update service")?;
    if response.status().as_u16() == 404 { return Err("尚未提供可用的更新清单 / No update manifest has been published yet".into()); }
    let bytes = response_bytes(response, 32768, |_| {}).await?;
    let release = decoded_release(&bytes)?;
    if source.fixture_origin.is_none() && !semver::Version::parse(&release.version).map_err(|_| "Invalid update version")?.pre.is_empty() {
        return Err("公开更新清单包含测试版本，已停止 / Public update feed contains a prerelease".into());
    }
    let url = url::Url::parse(&release.url).map_err(|_| "Invalid download URL")?;
    if !source.allows(&url) || (source.fixture_origin.is_none() && !release.url.starts_with(DOWNLOAD_PREFIX)) {
        return Err("更新下载来源无效 / Invalid update download source".into());
    }
    Ok(if newer(&release, version, build)? {Some(release)} else {None})
}
async fn download(source: &Source, release: &Release, progress: impl Fn(u64)) -> Result<Vec<u8>, String> {
    let response = source.client()?.get(&release.url).send().await.map_err(|_| "无法下载更新 / Could not download update")?;
    let bytes = response_bytes(response, release.size, progress).await?;
    if bytes.len() as u64 != release.size || format!("{:x}", Sha256::digest(&bytes)) != release.sha256.to_lowercase() {
        return Err("更新包校验失败，已停止 / Update archive verification failed".into());
    }
    Ok(bytes)
}
fn installed_app() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|_| "Could not locate application")?.canonicalize().map_err(|_| "Could not resolve application")?;
    let app = exe.parent().and_then(Path::parent).and_then(Path::parent).ok_or("Invalid app path")?;
    if app.extension().is_none_or(|s| s != "app") || exe != app.join("Contents/MacOS/vao") { return Err("请从已安装的 Vao.app 更新 / Run the installed Vao.app to update".into()); }
    Ok(app.to_owned())
}
fn validate_destination(target: &Path) -> Result<(), String> {
    if target.to_string_lossy().contains("/AppTranslocation/") || target.to_string_lossy().contains("/.Trash/") {
        return Err("请将 Vao 移至应用程序文件夹后再更新 / Move Vao to Applications before updating".into());
    }
    if target.is_symlink() || target.parent().is_none() { return Err("Invalid update destination".into()); }
    Ok(())
}
fn plist(app: &Path, field: &str) -> Result<String, String> {
    let result = std::process::Command::new("/usr/libexec/PlistBuddy").args(["-c", &format!("Print :{field}")]).arg(app.join("Contents/Info.plist")).output().map_err(|_| "Could not inspect app metadata")?;
    if !result.status.success() { return Err("更新包缺少应用身份 / Missing update app identity".into()); }
    String::from_utf8(result.stdout).map(|s| s.trim().to_string()).map_err(|_| "Invalid app metadata".into())
}
fn verify_app(app: &Path, release: &Release) -> Result<(), String> {
    for (field, expected) in [("CFBundleIdentifier",BUNDLE),("VaoReleaseVersion",release.version.as_str()),("CFBundleVersion",release.build.as_str()),("CFBundleExecutable","vao"),("LSMinimumSystemVersion","12.0")] {
        if plist(app, field)? != expected { return Err("更新应用身份或版本不匹配 / Update identity or version mismatch".into()); }
    }
    let mut magic = [0u8; 8];
    std::fs::File::open(app.join("Contents/MacOS/vao")).and_then(|mut f| f.read_exact(&mut magic)).map_err(|_| "Invalid update executable")?;
    if magic != [0xcf,0xfa,0xed,0xfe,0x0c,0,0,1] { return Err("Update requires an arm64 executable".into()); }
    for (path, id) in [(app.to_path_buf(),BUNDLE.to_string()), (app.join("Contents/Resources/vao-macos-bridge"),format!("{BUNDLE}.bridge"))] {
        let requirement = format!("=anchor apple generic and identifier \"{id}\" and certificate leaf[subject.OU] = \"{TEAM}\"");
        let result = std::process::Command::new("/usr/bin/codesign").args(["--verify","--deep","--strict","-R",&requirement]).arg(path).output().map_err(|_| "Could not verify Developer ID")?;
        if !result.status.success() { return Err("Developer ID 验证失败，已停止 / Developer ID verification failed".into()); }
    }
    let result = std::process::Command::new("/usr/sbin/spctl").args(["--assess","--type","execute"]).arg(app).output().map_err(|_| "Could not assess notarization")?;
    if !result.status.success() { return Err("更新包未通过系统安全检查，已停止 / Update did not pass Gatekeeper".into()); }
    Ok(())
}
fn prepare(bytes: &[u8], release: &Release, target: &Path) -> Result<tempfile::TempDir, String> {
    validate_destination(target)?;
    let stage = tempfile::Builder::new().prefix(".Vao-update-").tempdir_in(target.parent().ok_or("Invalid app parent")?)
        .map_err(|_| "无法写入应用所在目录，请将 Vao 移至应用程序文件夹 / Cannot write app folder; move Vao to Applications")?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    let mut total = 0u64; let mut count = 0usize;
    for item in archive.entries().map_err(|_| "Invalid update archive")? {
        let mut item = item.map_err(|_| "Invalid update entry")?;
        let path = item.path().map_err(|_| "Invalid update path")?.into_owned();
        let mut parts = path.components();
        if parts.next() != Some(Component::Normal("Vao.app".as_ref())) || parts.any(|p| !matches!(p,Component::Normal(_)))
            || !(item.header().entry_type().is_file() || item.header().entry_type().is_dir()) {
            return Err("更新包包含不安全的路径或链接 / Unsafe update archive path or link".into());
        }
        total = total.checked_add(item.size()).ok_or("Invalid archive size")?; count += 1;
        if total > MAX_EXTRACTED || count > 4096 { return Err("Expanded update exceeds limits".into()); }
        item.unpack_in(stage.path()).map_err(|_| "更新包解压失败 / Could not extract update")?;
    }
    verify_app(&stage.path().join("Vao.app"), release)?;
    Ok(stage)
}
fn replace(stage: &tempfile::TempDir, release: &Release, target: &Path) -> Result<PathBuf, String> {
    validate_destination(target)?;
    let new_app = stage.path().join("Vao.app");
    verify_app(&new_app, release)?;
    let backup = stage.path().join("previous.app");
    swap_bundles(target, &new_app, &backup)?;
    Ok(backup)
}
fn swap_bundles(target: &Path, new_app: &Path, backup: &Path) -> Result<(), String> {
    std::fs::rename(target, backup).map_err(|_| "无法替换应用，原版本保持 / Cannot replace app; previous version retained")?;
    if std::fs::rename(new_app, target).is_err() {
        return match std::fs::rename(backup, target) {
            Ok(()) => Err("安装失败，已恢复原版本 / Install failed; previous version restored".into()),
            Err(_) => Err(format!("安装失败，请从 {} 恢复 / Install failed; restore the backup", backup.display())),
        };
    }
    Ok(())
}
fn install_staged(stage: tempfile::TempDir, release: &Release, target: &Path) -> Result<PathBuf, String> {
    let result = replace(&stage, release, target);
    // Preserve a moved original even if restoring it fails; never delete the only remaining copy.
    if stage.path().join("previous.app").exists() { let _ = stage.keep(); }
    result
}

/// Explicit isolated diagnostic; never targets /Applications or production app data.
/// This overrides only this one CLI call, and trusts a certificate only in its own HTTP client.
pub async fn fixture(config: &Path) -> Result<serde_json::Value, String> {
    #[derive(Deserialize)] #[serde(deny_unknown_fields)]
    struct Fixture {endpoint: String, ca_pem: PathBuf, mode: String}
    let f: Fixture = serde_json::from_slice(&std::fs::read(config).map_err(|_| "Missing updater fixture")?).map_err(|_| "Invalid updater fixture")?;
    let root = config.parent().ok_or("Missing fixture directory")?.canonicalize().map_err(|_| "Invalid fixture directory")?;
    if !root.join(".vao-updater-fixture").is_file() || root.starts_with("/Applications") || root.starts_with("/System") || root.starts_with("/Library") { return Err("An explicit isolated updater fixture is required".into()); }
    let endpoint = url::Url::parse(&f.endpoint).map_err(|_| "Invalid fixture URL")?;
    if endpoint.scheme() != "https" || endpoint.host_str() != Some("127.0.0.1") { return Err("Fixture requires loopback HTTPS".into()); }
    let source = Source { endpoint: f.endpoint, fixture_origin: Some(endpoint.origin().ascii_serialization()), ca: Some(std::fs::read(f.ca_pem).map_err(|_| "Missing fixture CA")?) };
    let target = root.join("install/Vao.app");
    if target.canonicalize().map_err(|_| "Missing isolated app")? != target { return Err("Fixture target must not be a symlink".into()); }
    let version = plist(&target,"VaoReleaseVersion")?; let build = plist(&target,"CFBundleVersion")?;
    let release = check_feed(&source, version.clone(), build.clone()).await?;
    let Some(release) = release else {return Ok(serde_json::json!({"phase":"current","version":version,"build":build}));};
    if f.mode == "check" { return Ok(serde_json::json!({"phase":"available","release":release})); }
    if !matches!(f.mode.as_str(),"download"|"install") {return Err("Invalid fixture mode".into());}
    let bytes = download(&source, &release, |_| {}).await?;
    let stage = prepare(&bytes, &release, &target)?;
    if f.mode == "download" {return Ok(serde_json::json!({"phase":"ready","release":release}));}
    let backup = install_staged(stage, &release, &target)?;
    Ok(serde_json::json!({"phase":"installed","version":plist(&target,"VaoReleaseVersion")?,"build":plist(&target,"CFBundleVersion")?,"backup":backup}))
}


#[cfg(test)]
mod tests {
    use super::*;
    fn release() -> Release { decoded_release(include_bytes!("fixtures/updater-valid.json")).unwrap() }
    #[test]
    fn valid_signature_covers_version_url_and_hash() {
        let r = release(); assert_eq!(r.version,"0.2.3"); assert_eq!(r.size,1024);
        let mut envelope: serde_json::Value = serde_json::from_slice(include_bytes!("fixtures/updater-valid.json")).unwrap();
        let mut payload: serde_json::Value = serde_json::from_slice(&STANDARD.decode(envelope["signed"].as_str().unwrap()).unwrap()).unwrap();
        for field in ["version","url","sha256"] {
            let old=payload[field].clone(); payload[field]="tampered".into();
            envelope["signed"]=STANDARD.encode(serde_json::to_vec(&payload).unwrap()).into();
            assert!(decoded_release(&serde_json::to_vec(&envelope).unwrap()).is_err()); payload[field]=old;
        }
    }
    #[test]
    fn no_downgrades_and_build_reissues_only_at_same_version() {
        let r=release(); assert!(newer(&r,"0.2.3-intent.preview.10".into(),"20261023.9999".into()).unwrap());
        assert!(!newer(&r,"0.2.4".into(),"20261023.1".into()).unwrap());
        assert!(newer(&r,"0.2.3".into(),"20261023.2012".into()).unwrap());
        assert!(!newer(&r,"0.2.3".into(),"20261023.2013".into()).unwrap());
        assert!(!newer(&r,"0.2.3".into(),"20261024.1".into()).unwrap());
        assert!(build_number("../1").is_err()); assert!(build_number("1.2.3").is_err());
    }
    #[test]
    fn operations_cannot_overlap_or_install_over_active_voice() {
        let state=Updates::default(); let guard=state.claim().unwrap(); assert!(state.claim().is_err()); drop(guard);
        assert!(state.install(||false).unwrap_err().contains("语音"));
        assert!(!state.installing.load(Ordering::SeqCst)); assert!(state.claim().is_ok());
    }
    #[test]
    fn redirects_and_destinations_keep_transport_boundaries() {
        let source=Source::production();
        for u in ["http://github.com/a","https://evil.test/a","https://user@github.com/a","https://github.com.evil.test/a", "http://updates.vaovao.app/downloads/a", "https://user@updates.vaovao.app/downloads/a", "https://updates.vaovao.app.evil.test/a", "https://github.com/loewe1631/Vao/releases/download/v0.2.4/a"] {
            assert!(!source.allows(&url::Url::parse(u).unwrap()));
        }
        assert!(source.allows(&url::Url::parse(FEED).unwrap()));
        assert!(source.allows(&url::Url::parse(&format!("{DOWNLOAD_PREFIX}Vao-0.2.4-mac-arm64.app.tar.gz")).unwrap()));
        assert!(validate_destination(Path::new("/private/AppTranslocation/abc/Vao.app")).is_err());
        assert!(validate_destination(Path::new("/tmp/example/.Trash/Vao.app")).is_err());
    }
    #[test]
    fn signed_archive_links_are_rejected_before_filesystem_escape() {
        let dir=tempfile::tempdir().unwrap(); let target=dir.path().join("Vao.app");std::fs::create_dir(&target).unwrap();
        let mut compressed=flate2::write::GzEncoder::new(Vec::new(),flate2::Compression::default());
        { let mut builder=tar::Builder::new(&mut compressed);let mut h=tar::Header::new_gnu();h.set_entry_type(tar::EntryType::Symlink);h.set_size(0);h.set_mode(0o777);h.set_link_name("../../outside").unwrap();h.set_cksum();builder.append_data(&mut h,"Vao.app/Contents/unsafe",std::io::empty()).unwrap();builder.finish().unwrap(); }
        let bytes=compressed.finish().unwrap();assert!(prepare(&bytes,&release(),&target).unwrap_err().contains("链接"));
        assert!(target.is_dir());assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(),1);
    }
    #[test]
    fn second_rename_failure_restores_the_original_bundle() {
        let dir=tempfile::tempdir().unwrap();let target=dir.path().join("Vao.app");std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("marker"),b"old").unwrap();let backup=dir.path().join("previous.app");
        let error=swap_bundles(&target,&dir.path().join("missing.app"),&backup).unwrap_err();
        assert!(error.contains("已恢复"));assert!(!backup.exists());assert_eq!(std::fs::read(target.join("marker")).unwrap(),b"old");
    }
    #[test]
    fn invalid_staged_bundle_cannot_replace_original() {
        let dir=tempfile::tempdir().unwrap();let target=dir.path().join("Vao.app");std::fs::create_dir(&target).unwrap();std::fs::write(target.join("marker"),b"original").unwrap();
        let stage=tempfile::Builder::new().tempdir_in(dir.path()).unwrap();
        assert!(install_staged(stage,&release(),&target).is_err());
        assert_eq!(std::fs::read(target.join("marker")).unwrap(),b"original");
    }
}
