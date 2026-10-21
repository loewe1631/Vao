use crate::{audio, models, platform, realtime, trial, AppState, Phase};
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::{
    sync::OnceCell,
    time::{sleep, Duration},
};
fn listening(app: &tauri::AppHandle, id: u64) -> bool {
    let state = app.state::<AppState>();
    let r = state.runtime.lock().unwrap();
    r.session.id == id && r.session.phase == Phase::Listening
}
async fn finished(app: &tauri::AppHandle, id: u64) {
    loop {
        if !listening(app, id) {
            return;
        }
        sleep(Duration::from_millis(60)).await;
    }
}
pub async fn session_key(cell: &OnceCell<Result<String, String>>) -> Result<String, String> {
    loop {
        if let Some(v) = cell.get() {
            return v.clone();
        }
        sleep(Duration::from_millis(20)).await;
    }
}
pub fn start_enhancements(app: &tauri::AppHandle, id: u64) {
    let (settings, target, recording) = {
        let state = app.state::<AppState>();
        let r = state.runtime.lock().unwrap();
        if r.session.id != id || r.session.phase != Phase::Listening {
            return;
        }
        (
            r.settings.clone(),
            r.target.clone(),
            r.capture.as_ref().map(audio::Capture::reader),
        )
    };
    if !target["selection"].as_str().unwrap_or("").is_empty() {
        return;
    }
    let Ok((route,llm_route))=trial::routes(&settings) else{return;};
    let ordinary={let state=app.state::<AppState>();let r=state.runtime.lock().unwrap();r.session.id==id&&r.session.phase==Phase::Listening&&r.markers.is_empty()};
    if settings.input_context && ordinary && llm_route.config.protocol=="openai_chat" {
        let h = app.clone();
        let t = target.clone();
        let expanded = settings.expanded_context;
        tauri::async_runtime::spawn(async move {
            let value = tokio::select! {v=platform::input_context(&t,expanded)=>v,_=finished(&h,id)=>None};
            let state = h.state::<AppState>();
            let mut r = state.runtime.lock().unwrap();
            if r.session.id == id && r.session.phase == Phase::Listening && r.markers.is_empty() {
                r.input_context = value;
            }
        });
    }
    if !settings.live_preview {
        return;
    }
    let Some(recording) = recording else {
        return;
    };
    let access = Arc::new(OnceCell::new());
    {
        let state = app.state::<AppState>();
        let mut r = state.runtime.lock().unwrap();
        if r.session.id != id || r.session.phase != Phase::Listening {
            return;
        }
        r.asr_access = Some(access.clone());
        r.preview_status = "starting".into();
    }
    let cache = access.clone();
    let key_route = route.clone();
    tauri::async_runtime::spawn(async move {
        let value = tauri::async_runtime::spawn_blocking(move || trial::key(&key_route, "asr"))
            .await
            .unwrap_or_else(|_| Err("读取连接凭据失败".into()));
        let _ = cache.set(value);
    });
    if route.config.protocol=="openai_realtime" {
        let (stop_tx,stop_rx)=tokio::sync::oneshot::channel();
        let (mut result_tx,result_rx)=tokio::sync::oneshot::channel();
        {
            let state=app.state::<AppState>();let mut r=state.runtime.lock().unwrap();
            if r.session.id!=id||r.session.phase!=Phase::Listening{return;}
            r.realtime=Some(realtime::Handle{stop:stop_tx,result:result_rx});
        }
        let h=app.clone();
        tauri::async_runtime::spawn(async move {
            let task=async {
                let key=session_key(&access).await?;
                realtime::run(&route.config,&key,&settings.language,&settings.dictionary,recording,stop_rx,|text|set_preview(&h,id,"live",Some(text))).await
            };
            let result=tokio::select! {v=task=>v,_=crate::cancelled(&h,id)=>return,_=result_tx.closed()=>return};
            if result.is_err(){set_preview(&h,id,"unavailable",None);}
            let _=result_tx.send(result);
        });
        return;
    }
    let h = app.clone();
    tauri::async_runtime::spawn(async move {
        let key = tokio::select! {v=session_key(&access)=>v,_=finished(&h,id)=>return};
        let Ok(key) = key else {
            set_preview(&h, id, "unavailable", None);
            return;
        };
        let mut last = 0;
        let mut failures = 0;
        loop {
            tokio::select! {_=sleep(Duration::from_secs(4))=>{},_=finished(&h,id)=>return}
            let end = recording.len();
            if end < last + audio::RATE as usize * 3 {
                continue;
            }
            last = end;
            let start = end.saturating_sub(audio::RATE as usize * 12);
            let input = recording.clone();
            let wav = tokio::select! {v=tauri::async_runtime::spawn_blocking(move ||input.wav(start,end))=>match v{Ok(Ok(v))=>v,_=>{set_preview(&h,id,"unavailable",None);return;}},_=finished(&h,id)=>return};
            let response = tokio::select! {v=tokio::time::timeout(Duration::from_secs(12),models::transcribe(&route.config,&key,wav,&settings.language,&settings.dictionary))=>v,_=finished(&h,id)=>return};
            match response {
                Ok(Ok(text)) => {
                    failures = 0;
                    let chars: Vec<char> = text.chars().collect();
                    let text = chars[chars.len().saturating_sub(400)..]
                        .iter()
                        .collect::<String>();
                    set_preview(&h, id, "live", Some(text));
                }
                Ok(Err(e)) if e.no_speech => {}
                _ => {
                    failures += 1;
                    if failures >= 2 {
                        set_preview(&h, id, "unavailable", None);
                        return;
                    }
                }
            }
        }
    });
}
fn set_preview(app: &tauri::AppHandle, id: u64, status: &str, text: Option<String>) {
    {
        let state = app.state::<AppState>();
        let mut r = state.runtime.lock().unwrap();
        if !r.accept_preview(id, status, text) {
            return;
        }
    }
    let _ = app.emit("session-updated", ());
}
