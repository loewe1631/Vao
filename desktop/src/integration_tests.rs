//! Synthetic protocol and storage tests. No microphone, credentials or production services.
use crate::{audio, models, store::*};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
};
use vao_core::{Layout, Profile, Scene};

fn server(
    responses: Vec<(&'static str, String)>,
) -> (String, Arc<Mutex<Vec<String>>>, std::thread::JoinHandle<()>) {
    server_delayed(responses, std::time::Duration::ZERO)
}
fn server_delayed(
    responses: Vec<(&'static str, String)>,
    delay: std::time::Duration,
) -> (String, Arc<Mutex<Vec<String>>>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(vec![]));
    let capture = requests.clone();
    let thread = std::thread::spawn(move || {
        for (status, body) in responses {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = vec![];
            let mut temp = [0u8; 8192];
            let split;
            loop {
                let n = socket.read(&mut temp).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&temp[..n]);
                if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    split = i + 4;
                    break;
                }
            }
            let headers = String::from_utf8_lossy(&bytes[..split]).to_lowercase();
            let size = headers
                .lines()
                .find_map(|l| {
                    l.strip_prefix("content-length:")
                        .map(|s| s.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            while bytes.len() < split + size {
                let n = socket.read(&mut temp).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&temp[..n]);
            }
            capture
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(&bytes).into_owned());
            std::thread::sleep(delay);
            write!(socket,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
    });
    (base, requests, thread)
}

#[tokio::test]
async fn built_in_writing_policy_reaches_the_chat_model_as_system_instructions() {
    let (base, requests, thread) = server(vec![("200 OK", r#"{"choices":[{"message":{"content":"固定协议返回"}}]}"#.into())]);
    let text = "开发机的 System D 服务正常，版本 2.4.1，线上没验证，有 Time Out 报错，可能是网络问题。";
    let plan = vao_core::WritingPlan::resolve(text, "Codex", &[], &Profile::default());
    let config = ConnectionConfig { base, model: "synthetic".into(), protocol: "openai_chat".into() };
    models::refine(&config, "", text, &plan.prompt(&[], None, None), None, &[]).await.unwrap_or_else(|e| panic!("{}", e.message));
    thread.join().unwrap();
    let capture = requests.lock().unwrap();
    let body: serde_json::Value = serde_json::from_str(capture[0].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["messages"][0]["role"], "system");
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(system.contains("技术记录模板"));
    assert!(system.contains("未验证"));
    assert!(system.contains("另起自然段"));
    let data: serde_json::Value = serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(data["transcript"], text);
    assert_eq!(data["operation"], "dictation_cleanup");
}

#[tokio::test]
async fn restricted_refine_protocol_also_receives_the_selected_preset() {
    let (base, requests, thread) = server(vec![("200 OK", r#"{"text":"固定协议返回"}"#.into())]);
    let text = "开发机测试正常，线上未验证，先记录下来。";
    let plan = vao_core::WritingPlan::resolve(text, "Codex", &[], &Profile::default());
    let config = ConnectionConfig { base, model: "synthetic".into(), protocol: "vao_refine".into() };
    models::refine(&config, "", text, &plan.prompt(&[], None, None), None, &plan.preferences(None)).await.unwrap_or_else(|e| panic!("{}", e.message));
    thread.join().unwrap();
    let capture = requests.lock().unwrap();
    assert!(capture[0].starts_with("POST /v1/refine"));
    let body: serde_json::Value = serde_json::from_str(capture[0].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["text"], text);
    let prefs = body["preferences"].as_array().unwrap();
    assert!(prefs.iter().any(|p| p.as_str().unwrap().contains("技术记录模板")));
    assert!(prefs.iter().map(|p| p.as_str().unwrap().chars().count()).sum::<usize>() <= 800);
}

#[tokio::test]
async fn full_refine_url_preserves_the_legacy_contract() {
    let (base, requests, thread) = server(vec![("200 OK", r#"{"text":"后天开会。"}"#.into())]);
    let config = ConnectionConfig {
        base: format!("{base}/refine"),
        model: "synthetic".into(),
        protocol: "vao_refine".into(),
    };
    let result = models::test_connection(&config, "").await.unwrap_or_else(|e| panic!("{}", e.message));
    assert_eq!(result.output, "后天开会。");
    thread.join().unwrap();
    let capture = requests.lock().unwrap();
    assert_eq!(capture.len(), 1);
    assert!(capture[0].starts_with("POST /v1/refine HTTP/1.1"));
    let body: serde_json::Value = serde_json::from_str(capture[0].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["text"], result.input);
    assert!(body.get("preferences").is_some());
    assert!(body.get("messages").is_none());
}

#[tokio::test]
async fn unsupported_refine_route_explains_protocol_without_retrying_or_echoing_body() {
    for status in ["404 Not Found", "405 Method Not Allowed"] {
        let (base, requests, thread) = server(vec![(status, r#"{"error":{"message":"synthetic-private-provider-detail"}}"#.into())]);
        let config = ConnectionConfig { base, model: "synthetic".into(), protocol: "vao_refine".into() };
        let error = models::test_connection(&config, "").await.err().expect("unsupported route must fail");
        assert!(error.message.contains("OpenAI Chat Completions"));
        assert!(error.message.contains(&status[..3]));
        assert!(!error.message.contains("synthetic-private-provider-detail"));
        assert!(!error.retryable);
        thread.join().unwrap();
        let capture = requests.lock().unwrap();
        assert_eq!(capture.len(), 1);
        assert!(capture[0].starts_with("POST /v1/refine HTTP/1.1"));
    }
}

#[tokio::test]
async fn refine_does_not_accept_a_chat_response_as_its_own_protocol() {
    let (base, requests, thread) = server(vec![("200 OK", r#"{"choices":[{"message":{"content":"后天开会。"}}]}"#.into())]);
    let config = ConnectionConfig { base, model: "synthetic".into(), protocol: "vao_refine".into() };
    let error = models::test_connection(&config, "").await.err().expect("wrong response must fail");
    assert!(error.message.contains("JSON text"));
    assert!(error.message.contains("OpenAI Chat Completions"));
    assert!(!error.retryable);
    thread.join().unwrap();
    assert_eq!(requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn refine_auth_failure_keeps_credential_diagnosis() {
    let (base, _, thread) = server(vec![("401 Unauthorized", "{}".into())]);
    let config = ConnectionConfig { base, model: "synthetic".into(), protocol: "vao_refine".into() };
    let error = models::test_connection(&config, "").await.err().expect("unauthorized must fail");
    assert!(error.message.contains("Credential rejected"));
    assert!(!error.message.contains("choose OpenAI"));
    thread.join().unwrap();
}

#[tokio::test]
async fn lost_date_gets_one_repair_using_the_original_transcript() {
    let (base, requests, thread) = server(vec![
        ("200 OK", r#"{"choices":[{"message":{"content":"寄出合同。"}}]}"#.into()),
        ("200 OK", r#"{"choices":[{"message":{"content":"星期二寄出合同。"}}]}"#.into()),
    ]);
    let text = "星期二寄出合同。";
    let plan = vao_core::WritingPlan::resolve(text,"Codex",&[],&Profile::default());
    let config = ConnectionConfig { base, model:"synthetic".into(), protocol:"openai_chat".into() };
    let request = models::WritingRequest { text, plan:&plan, dictionary:&[],translation:None,selection:None,options:vao_core::WritingOptions{semantic_completion:false} };
    let output=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{}",e.message));
    assert!(output.repair_attempted);assert!(!output.used_original);assert_eq!(output.text,text);
    thread.join().unwrap();let capture=requests.lock().unwrap();assert_eq!(capture.len(),2);
    let body:serde_json::Value=serde_json::from_str(capture[1].split_once("\r\n\r\n").unwrap().1).unwrap();
    let data:serde_json::Value=serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();assert_eq!(data["transcript"],text);
}

#[tokio::test]
async fn repeated_quote_tail_loss_preserves_the_source_without_a_third_attempt() {
    let body=r#"{"choices":[{"message":{"content":"“先两份，不对，三份。”"}}]}"#;
    let (base,requests,thread)=server(vec![("200 OK",body.into()),("200 OK",body.into())]);
    let text="原样保留：“先两份，不对，三份。”这是我的说明。";
    let plan=vao_core::WritingPlan::resolve(text,"Codex",&[],&Profile::default());
    let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
    let request=models::WritingRequest{text,plan:&plan,dictionary:&[],translation:None,selection:None,options:vao_core::WritingOptions{semantic_completion:false}};
    let output=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{}",e.message));
    assert!(output.repair_attempted);assert!(output.used_original);assert_eq!(output.text,text);
    thread.join().unwrap();assert_eq!(requests.lock().unwrap().len(),2);
}

#[tokio::test]
async fn repair_preserves_both_the_date_and_the_required_numbered_steps() {
    let (base,requests,thread)=server(vec![
        ("200 OK",r#"{"choices":[{"message":{"content":"星期二先寄合同，再确认签收，最后更新交接表。"}}]}"#.into()),
        ("200 OK",r#"{"choices":[{"message":{"content":"星期二：\n1. 寄合同。\n2. 确认签收。\n3. 更新交接表。"}}]}"#.into()),
    ]);
    let text="星期二先寄合同，再确认签收，最后更新交接表。";
    let plan=vao_core::WritingPlan::resolve(text,"Codex",&[],&Profile::default());
    let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
    let request=models::WritingRequest{text,plan:&plan,dictionary:&[],translation:None,selection:None,options:vao_core::WritingOptions{semantic_completion:false}};
    let output=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{}",e.message));
    assert!(output.repair_attempted);assert!(!output.used_original);assert!(output.text.contains("3. 更新交接表"));
    thread.join().unwrap();assert_eq!(requests.lock().unwrap().len(),2);
}

#[tokio::test]
async fn translation_and_selected_text_bypass_dictation_guards() {
    let (base,requests,thread)=server(vec![
        ("200 OK",r#"{"choices":[{"message":{"content":"Send it on Tuesday."}}]}"#.into()),
        ("200 OK",r#"{"choices":[{"message":{"content":"{\"allowed\":true}"},"finish_reason":"stop"}]}"#.into()),
        ("200 OK",r#"{"choices":[{"message":{"content":"一份。"}}]}"#.into()),
    ]);
    let text="星期二发送。";let plan=vao_core::WritingPlan::resolve(text,"Codex",&[],&Profile::default());
    let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
    for (translation,selection) in [(Some("English"),None),(None,Some("选中原文"))] {
        let request=models::WritingRequest{text,plan:&plan,dictionary:&[],translation,selection,options:vao_core::WritingOptions{semantic_completion:false}};
        let output=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{}",e.message));
        assert!(!output.repair_attempted);assert!(!output.used_original);
    }
    thread.join().unwrap();assert_eq!(requests.lock().unwrap().len(),3);
}

#[test]
fn replay_snapshot_is_read_only_and_preserves_preferences() {
    let path = std::env::temp_dir().join(format!("vao-read-only-{}-{}.sqlite3", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    {
        let store = Store::open(&path).unwrap();
        let mut profile = Profile::default();
        profile.feedback(1, Scene::Tasks, Layout::Paragraph, true).unwrap();
        store.set("profile", &profile).unwrap();
    }
    {
        let snapshot = Store::open_read_only(&path).unwrap();
        let profile: Profile = snapshot.get("profile");
        assert_eq!(profile.layout(Scene::Tasks, None), Layout::Paragraph);
        assert!(snapshot.set("profile", &Profile::default()).is_err());
    }
    let snapshot = Store::open_read_only(&path).unwrap();
    assert_eq!(snapshot.get::<Profile>("profile").learned_revision, 1);
    drop(snapshot);
    std::fs::remove_file(path).unwrap();
}
#[tokio::test]
async fn synthetic_audio_to_asr_to_refinement() {
    let (base, requests, thread) = server(vec![
        (
            "200 OK",
            r#"{"data":[{"id":"fake-asr"},{"id":"fake-text"}]}"#.into(),
        ),
        ("200 OK", r#"{"text":"明天开会，不对，后天开会。"}"#.into()),
        (
            "200 OK",
            r#"{"choices":[{"message":{"content":"后天开会。"}}]}"#.into(),
        ),
    ]);
    let asr = ConnectionConfig {
        base: base.clone(),
        model: "fake-asr".into(),
        protocol: "openai_audio".into(),
    };
    assert_eq!(models::list(&asr, "synthetic-key").await.unwrap().len(), 2);
    let pcm = (0..16000)
        .map(|i| ((i as f32) * 0.05).sin() * 0.1)
        .collect::<Vec<_>>();
    let transcript = models::transcribe(
        &asr,
        "synthetic-key",
        audio::wav(&pcm, 16000),
        "zh",
        &["Vao".into()],
    )
    .await
    .unwrap_or_else(|e| panic!("{}", e.message));
    let llm = ConnectionConfig {
        base,
        model: "fake-text".into(),
        protocol: "openai_chat".into(),
    };
    let output = models::refine(
        &llm,
        "synthetic-key",
        &transcript,
        "保留最终改口",
        None,
        &[],
    )
    .await
    .unwrap_or_else(|e| panic!("{}", e.message));
    assert_eq!(output, "后天开会。");
    thread.join().unwrap();
    let captured = requests.lock().unwrap();
    assert!(captured[1].starts_with("POST /v1/audio/transcriptions"));
    assert!(captured[1].contains("audio/wav"));
    assert!(captured[1].contains("fake-asr"));
    assert!(captured[1].contains("Vao"));
    assert!(captured[2].contains("fake-text"));
    assert!(captured[2].contains("明天开会"));
}
#[tokio::test]
async fn provider_errors_not_mislabelled_success() {
    let (base, _, thread) = server(vec![
        ("401 Unauthorized", "{}".into()),
        ("429 Too Many Requests", "{}".into()),
        (
            "200 OK",
            r#"{"choices":[{"message":{"content":""}}]}"#.into(),
        ),
    ]);
    let c = ConnectionConfig {
        base,
        model: "fake".into(),
        protocol: "openai_chat".into(),
    };
    let a = models::refine(&c, "", "a", "b", None, &[])
        .await
        .err()
        .unwrap();
    assert!(!a.retryable);
    assert!(a.message.contains("401"));
    let b = models::refine(&c, "", "a", "b", None, &[])
        .await
        .err()
        .unwrap();
    assert!(b.retryable);
    assert!(b.message.contains("429"));
    assert!(models::refine(&c, "", "a", "b", None, &[]).await.is_err());
    thread.join().unwrap();
}
#[tokio::test]
async fn malformed_asr_response_is_failure() {
    let (base, _, thread) = server(vec![("200 OK", r#"{"unexpected":"data"}"#.into())]);
    let c = ConnectionConfig {
        base,
        model: "fake".into(),
        protocol: "openai_audio".into(),
    };
    assert!(
        models::transcribe(&c, "", audio::wav(&vec![0.; 8000], 16000), "auto", &[])
            .await
            .is_err()
    );
    thread.join().unwrap();
}

#[tokio::test]
async fn all_json_responses_reject_oversized_bodies() {
    let body = serde_json::json!({"padding": "x".repeat(524289)}).to_string();
    let (base, _, thread) = server(vec![("200 OK", body)]);
    let response = models::client().unwrap().get(base).send().await.unwrap();
    let error = models::read(response).await.err().unwrap();
    assert!(error.message.contains("超过限制"));
    assert!(!error.retryable);
    thread.join().unwrap();
}

#[tokio::test]
async fn redirects_do_not_forward_provider_credentials() {
    let destination = TcpListener::bind("127.0.0.1:0").unwrap();
    destination.set_nonblocking(true).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/models", listener.local_addr().unwrap());
    let location = format!("http://{}/destination", destination.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut request = vec![];
        let mut buffer = [0_u8; 1024];
        while !request.windows(4).any(|x| x == b"\r\n\r\n") {
            let count = socket.read(&mut buffer).unwrap();
            assert!(count > 0);
            request.extend_from_slice(&buffer[..count]);
        }
        assert!(String::from_utf8_lossy(&request).contains("Bearer synthetic-redirect-key"));
        write!(socket, "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}").unwrap();
    });
    let response = models::client().unwrap().get(url).bearer_auth("synthetic-redirect-key").send().await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::FOUND);
    assert!(models::read(response).await.is_err());
    thread.join().unwrap();
    assert_eq!(destination.accept().err().unwrap().kind(), std::io::ErrorKind::WouldBlock);
}


#[tokio::test]
async fn empty_asr_has_a_typed_notice_but_other_errors_remain_failures() {
    let (base, _, thread) = server(vec![
        (
            "422 Unprocessable Entity",
            r#"{"error":{"code":"empty_transcript"}}"#.into(),
        ),
        ("200 OK", r#"{"text":"  "}"#.into()),
        ("401 Unauthorized", "{}".into()),
    ]);
    let c = ConnectionConfig {
        base,
        model: "synthetic".into(),
        protocol: "openai_audio".into(),
    };
    for expected in [true, true, false] {
        let e = models::transcribe(&c, "", audio::wav(&vec![0.01; 8000], 16000), "auto", &[])
            .await
            .err()
            .unwrap();
        assert_eq!(e.no_speech, expected);
        let flow = crate::input_feedback::FlowError::from(e);
        assert_eq!(
            matches!(flow, crate::input_feedback::FlowError::Notice(_)),
            expected
        );
    }
    thread.join().unwrap();
}

#[tokio::test]
async fn selection_and_voice_instruction_reach_the_model_separately() {
    let (base, requests, thread) = server(vec![
        ("200 OK",r#"{"choices":[{"message":{"content":"{\"allowed\":true}"},"finish_reason":"stop"}]}"#.into()),
    (
        "200 OK",
        r#"{"choices":[{"message":{"content":"请后天发送报告。"}}]}"#.into(),
    )]);
    let c = ConnectionConfig {
        base,
        model: "synthetic".into(),
        protocol: "openai_chat".into(),
    };
    let selected = "请明天发送报告。";
    let instruction = "时间改成后天";
    let prompt = vao_core::system_prompt(Scene::Chat, Layout::Paragraph, &[], None, Some(selected));
    assert_eq!(
        models::refine(&c, "", instruction, &prompt, Some(selected), &[])
            .await
            .unwrap_or_else(|e| panic!("{e}")),
        "请后天发送报告。"
    );
    thread.join().unwrap();
    let r = requests.lock().unwrap();
    let body = r[1].split("\r\n\r\n").nth(1).unwrap();
    let v: serde_json::Value = serde_json::from_str(body).unwrap();
    let content: serde_json::Value =
        serde_json::from_str(v["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(content["selected_text"], selected);
    assert_eq!(content["voice_instruction"], instruction);
}
#[test]
fn database_feedback_round_trip_and_retention() {
    let mut db = Store::open(std::path::Path::new(":memory:")).unwrap();
    for _ in 0..3 {
        let r = db
            .add(ResultRecord {
                id: 0,
                created: crate::now(),
                scene: Scene::Chat,
                layout: Layout::Paragraph,
                revision: 0,
                original: "合成原文".into(),
                text: "合成结果".into(),
                risks: vec![],
                app: "Synthetic".into(),
                delivery: "preview".into(),
                feedback: false,
                refinement: vec![],
            })
            .unwrap();
        db.feedback(r.id, Layout::Numbered, true).unwrap();
        assert!(db.feedback(r.id, Layout::Numbered, true).is_err());
    }
    let mut p: Profile = db.get("profile");
    assert_eq!(p.layout(Scene::Chat, None), Layout::Numbered);
    assert_eq!(db.history().unwrap().len(), 3);
    p.forget();
    db.set("profile", &p).unwrap();
    assert!(db.get::<Profile>("profile").votes.is_empty());
    assert!(db.get::<Profile>("profile").rules.is_empty());
    db.clear_history().unwrap();
    assert!(db.history().unwrap().is_empty());
    let r = db
        .add(ResultRecord {
            id: 0,
            created: crate::now() - 604801,
            scene: Scene::Chat,
            layout: Layout::Paragraph,
            revision: 0,
            original: "expired".into(),
            text: "expired".into(),
            risks: vec![],
            app: "Synthetic".into(),
            delivery: "preview".into(),
            feedback: false,
                refinement: vec![],
        })
        .unwrap();
    assert!(db.result(r.id).is_err());
}

#[test]
fn shortcut_settings_survive_reopen_and_legacy_settings_keep_models() {
    let old = r#"{"asr":{"base":"http://127.0.0.1:9000/v1","model":"synthetic-asr","protocol":"openai_audio"},"dictionary":["SyntheticTerm"]}"#;
    let mut settings: Settings = serde_json::from_str(old).unwrap();
    assert_eq!(settings.shortcuts, crate::hotkeys::Shortcuts::default());
    assert!(settings.auto_input); // Missing fields receive the new-install default.
    assert!(!settings.auto_stop);
    assert_eq!(settings.silence_seconds, 5);
    let explicit_preview: Settings = serde_json::from_str(r#"{"auto_input":false}"#).unwrap();
    assert!(!explicit_preview.auto_input); // Upgrades preserve the user's choice.
    settings.auto_input = false;
    settings.auto_stop = true;
    settings.silence_seconds = 7;
    settings.shortcuts.record = "Control+Shift+KeyR".into();
    let path = std::env::temp_dir().join(format!(
        "vao-shortcut-test-{}-{}.sqlite3",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    {
        let db = Store::open(&path).unwrap();
        db.set("settings", &settings).unwrap();
    }
    {
        let db = Store::open(&path).unwrap();
        let restored: Settings = db.get("settings");
        assert!(!restored.auto_input);
        assert!(restored.auto_stop);
        assert_eq!(restored.silence_seconds, 7);
        assert_eq!(restored.shortcuts, settings.shortcuts);
        assert_eq!(restored.asr.model, "synthetic-asr");
        assert_eq!(restored.dictionary, vec!["SyntheticTerm"]);
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn auto_stop_delay_rejects_invalid_values_without_changing_other_settings() {
    let mut settings = Settings::default();
    for seconds in [0, 1, 31, 255] {
        settings.silence_seconds = seconds;
        assert!(crate::validate_settings(&settings).is_err());
    }
    for seconds in [2, 5, 17, 30] {
        settings.silence_seconds = seconds;
        assert!(crate::validate_settings(&settings).is_ok());
    }
    assert!(!settings.auto_stop);
}

#[test]
fn automatic_and_manual_stop_claim_one_session_and_reject_stale_work() {
    let mut r = crate::Runtime::default();
    let id = r.session.begin().unwrap();
    r.session.advance(id, vao_core::Phase::Listening).unwrap();
    assert!(r.claim_finish(id));
    assert!(!r.claim_finish(id));
    r.session.cancel();
    assert!(!r.claim_finish(id));
    let next = r.session.begin().unwrap();
    r.session.advance(next, vao_core::Phase::Listening).unwrap();
    assert!(!r.claim_finish(id));
    assert_eq!(r.session.phase, vao_core::Phase::Listening);
    assert!(r.claim_finish(next));
}

#[test]
fn concurrent_stop_requests_have_one_winner() {
    let mut r = crate::Runtime::default();
    let id = r.session.begin().unwrap();
    r.session.advance(id, vao_core::Phase::Listening).unwrap();
    let runtime = std::sync::Mutex::new(r);
    let barrier = std::sync::Barrier::new(3);
    let winners = std::thread::scope(|scope| {
        let stop = || {
            barrier.wait();
            runtime.lock().unwrap().claim_finish(id)
        };
        let manual = scope.spawn(stop);
        let automatic = scope.spawn(stop);
        barrier.wait();
        usize::from(manual.join().unwrap()) + usize::from(automatic.join().unwrap())
    });
    assert_eq!(winners, 1);
    assert_eq!(runtime.lock().unwrap().session.phase, vao_core::Phase::Processing);
}

#[test]
fn profile_switch_roundtrip_preserves_effective_version() {
    let db = Store::open(std::path::Path::new(":memory:")).unwrap();
    let mut p = Profile::default();
    for id in 1..=3 {
        p.feedback(id, Scene::Chat, Layout::Numbered, true).unwrap();
    }
    let version = p.learned_revision;
    db.set("profile", &p).unwrap();
    for enabled in [false, true, false, true] {
        let mut loaded: Profile = db.get("profile");
        loaded.set_learning(enabled);
        loaded.set_personalization(enabled);
        db.set("profile", &loaded).unwrap();
    }
    let loaded: Profile = db.get("profile");
    assert_eq!(loaded.learned_revision, version);
    assert_eq!(loaded.rules.len(), 1);
    assert_eq!(loaded.votes.len(), 3);
}

#[tokio::test]
async fn connection_test_reports_complete_roundtrip_and_propagates_errors() {
    let (base, requests, thread) = server_delayed(
        vec![
            (
                "200 OK",
                r#"{"choices":[{"message":{"content":"后天开会。"}}]}"#.into(),
            ),
            ("503 Service Unavailable", "{}".into()),
        ],
        std::time::Duration::from_millis(45),
    );
    let c = ConnectionConfig {
        base,
        model: "synthetic-test-model".into(),
        protocol: "openai_chat".into(),
    };
    let result = models::test_connection(&c, "synthetic-test-key")
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(result.input, "明天开会，不对，后天开会。");
    assert_eq!(result.output, "后天开会。");
    assert_eq!(result.model, c.model);
    assert!(result.latency_ms >= 40);
    assert!(models::test_connection(&c, "synthetic-test-key")
        .await
        .is_err());
    thread.join().unwrap();
    let requests = requests.lock().unwrap();
    assert!(requests[0].starts_with("POST /v1/chat/completions"));
    assert!(requests[0].contains("明天开会，不对，后天开会。"));
}

#[test]
fn selection_answer_completes_before_the_next_recording_in_the_same_app() {
    use vao_core::Phase;
    let mut runtime = crate::Runtime::default();
    let id = runtime.session.begin().unwrap();
    for phase in [Phase::Listening, Phase::Processing, Phase::Ready] {
        runtime.session.advance(id, phase).unwrap();
    }
    runtime.target = serde_json::json!({"pid":123,"selection":"合成选区"});
    // Even before the answer is shown, Fn must not paste a selected-text result.
    assert!(!runtime.pending_delivery());
    runtime.present_answer(id, "合成解释").unwrap();
    assert_eq!(runtime.session.phase, Phase::Completed);
    assert!(!runtime.session.delivered);
    assert!(!runtime.session.active(id));
    assert!(!runtime.pending_delivery());
    assert!(runtime.session.claim_delivery(id).is_err());
    // Dismissing the answer or moving the caret must not revive the old delivery.
    runtime.answer.clear();
    assert!(!runtime.pending_delivery());
    let next = runtime.session.begin().unwrap();
    assert!(next > id);
    runtime.session.advance(next, Phase::Listening).unwrap();
    assert!(runtime.present_answer(id, "迟到的旧答案").is_err());
    assert_eq!(runtime.session.phase, Phase::Listening);
    assert!(runtime.answer.is_empty());
}

#[test]
fn ordinary_preview_still_allows_explicit_delivery_and_pasted_edits_stay_terminal() {
    use vao_core::Phase;
    let mut runtime = crate::Runtime::default();
    let id = runtime.session.begin().unwrap();
    for phase in [Phase::Listening, Phase::Processing, Phase::Ready] {
        runtime.session.advance(id, phase).unwrap();
    }
    assert!(runtime.pending_delivery());
    runtime.session.claim_delivery(id).unwrap();
    runtime.session.advance(id, Phase::Completed).unwrap();
    runtime.present_answer(id, "合成修改结果").unwrap();
    assert!(runtime.session.delivered);
    assert!(!runtime.pending_delivery());
    assert!(runtime.session.claim_delivery(id).is_err());
    assert!(runtime.session.begin().is_ok());
}


#[tokio::test]
async fn dictated_question_and_selection_question_have_distinct_contracts() {
    let (base, requests, thread) = server(vec![
        ("200 OK", r#"{"choices":[{"message":{"content":"我能发布软件吗？"}}]}"#.into()),
        ("200 OK",r#"{"choices":[{"message":{"content":"{\"allowed\":true}"},"finish_reason":"stop"}]}"#.into()),
        ("200 OK", r#"{"choices":[{"message":{"content":"这是选区的解释。"}}]}"#.into()),
    ]);
    let c=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
    for selection in [None,Some("原文")] {
        let prompt=vao_core::system_prompt(Scene::Tasks,Layout::Paragraph,&[],None,selection);
        models::refine(&c,"","我能发布软件吗？",&prompt,selection,&[]).await.unwrap_or_else(|e|panic!("{}",e.message));
    }
    thread.join().unwrap();
    let captured=requests.lock().unwrap();
    let body=|i:usize|serde_json::from_str::<serde_json::Value>(captured[i].split("\r\n\r\n").nth(1).unwrap()).unwrap();
    let ordinary=body(0);let selected=body(2);
    let content=|v:&serde_json::Value|serde_json::from_str::<serde_json::Value>(v["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(content(&ordinary)["operation"],"dictation_cleanup");
    assert_eq!(content(&ordinary)["transcript"],"我能发布软件吗？");
    assert!(content(&ordinary)["selected_text"].is_null());
    assert_eq!(content(&selected)["selected_text"],"原文");
    assert_eq!(content(&selected)["voice_instruction"],"我能发布软件吗？");
}

#[test]
fn partial_recording_notice_preserves_phase_and_does_not_hide_failure_or_answer() {
    use crate::{input_feedback::InputNotice, Runtime, Phase};
    let mut r = Runtime::default();
    r.session.id = 6;
    r.session.phase = Phase::Completed;
    assert!(r.recording_warning(5).is_none());
    assert!(r.recording_warning(6).is_some());
    assert_eq!(r.session.phase, Phase::Completed);
    assert_eq!(r.notice, Some(InputNotice::RecordingInterrupted));
    r.notice = Some(InputNotice::DeliveryTargetUnavailable);
    assert!(r.recording_warning(6).is_none());
    assert_eq!(r.notice, Some(InputNotice::DeliveryTargetUnavailable));
    r.notice = None;
    r.answer = "answer".into();
    assert!(r.recording_warning(6).is_none());
    r.answer.clear();
    r.session.phase = Phase::Listening;
    assert!(r.recording_warning(6).is_none());
    r.session.phase = Phase::Failed;
    assert!(r.recording_warning(6).is_none());
}

#[tokio::test]
async fn speech_file_diagnostic_sends_fixed_wav_and_rejects_invalid_transcripts() {
    use base64::Engine;
    for (status, body, ok) in [
        ("200 OK", r#"{"text":"This is a speech recognition test."}"#, true),
        ("200 OK", r#"{"text":"  "}"#, false),
        ("200 OK", r#"{"result":"not a transcript"}"#, false),
        ("401 Unauthorized", r#"{"error":{"message":"synthetic-private-provider-detail"}}"#, false),
    ] {
        let (base, requests, thread) = server(vec![(status, body.into())]);
        let config = ConnectionConfig { base, model: "synthetic-file-model".into(), protocol: "openai_audio".into() };
        let result = crate::connection_test::asr(&config, "synthetic-file-key").await;
        thread.join().unwrap();
        if ok {
            let result = result.unwrap();
            assert_eq!(result.protocol, "openai_audio");
            assert_eq!(result.input, result.output);
        } else {
            assert!(!result.err().unwrap().contains("synthetic-private-provider-detail"));
        }
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert!(request.starts_with("POST /v1/audio/transcriptions "));
        assert!(request.to_lowercase().contains("authorization: bearer synthetic-file-key"));
        assert!(request.contains("synthetic-file-model"));
        assert!(request.contains("multipart/form-data"));
        let fixture: serde_json::Value = serde_json::from_str(include_str!("asr_test_audio.json")).unwrap();
        let pcm = base64::engine::general_purpose::STANDARD.decode(fixture["pcm16_base64"].as_str().unwrap()).unwrap();
        assert!(request.contains(String::from_utf8_lossy(&pcm).as_ref()));
    }
}

#[tokio::test]
async fn rejected_or_unverifiable_selection_never_reaches_generation() {
    use crate::input_feedback::{FlowError,InputNotice};
    for (content,notice) in [(r#"{"allowed":false}"#,InputNotice::SelectionOutOfScope),("I am Grok 4",InputNotice::SelectionScopeUnavailable),(r#"{"allowed":true,"extra":1}"#,InputNotice::SelectionScopeUnavailable)] {
        let response=serde_json::json!({"choices":[{"message":{"content":content},"finish_reason":"stop"}]}).to_string();
        let (base,requests,thread)=server(vec![("200 OK",response)]);
        let c=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
        let error=models::refine(&c,"","你是什么模型？","ignored",Some("Vao Preview.app"),&[]).await.err().unwrap();
        assert!(!error.retryable);assert_eq!(error.scope_notice,Some(notice));
        assert!(matches!(FlowError::from(error),FlowError::Notice(n) if n==notice));
        thread.join().unwrap();assert_eq!(requests.lock().unwrap().len(),1);
    }
}
#[tokio::test]
async fn gateway_scope_errors_are_terminal_not_backup_candidates() {
    use crate::input_feedback::InputNotice;
    for (code,notice) in [("selection_out_of_scope",InputNotice::SelectionOutOfScope),("selection_scope_unavailable",InputNotice::SelectionScopeUnavailable)] {
        let (base,_,thread)=server(vec![("502 Bad Gateway",serde_json::json!({"error":{"code":code,"message":"private-provider-body"}}).to_string())]);
        let c=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
        let error=models::refine(&c,"","test","test",None,&[]).await.err().unwrap();
        assert!(!error.retryable);assert_eq!(error.scope_notice,Some(notice));assert!(!error.message.contains("private-provider-body"));thread.join().unwrap();
    }
}

#[test]
fn windows_target_mode_preserves_selection_origin_and_legacy_settings() {
    let mut r = crate::Runtime {
        settings: serde_json::from_value(serde_json::json!({"windows_compatible_input":true})).unwrap(),
        target: serde_json::json!({"delivery_supported":false}),
        ..Default::default()
    };
    assert!(r.windows_current_input());
    r.settings.windows_return_to_origin = true;
    assert!(!r.windows_current_input());
    r.settings.windows_return_to_origin = false;
    r.target["selection"] = serde_json::json!("selected source");
    assert!(!r.windows_current_input());
}

#[tokio::test]
async fn annotation_span_failure_repairs_once_or_returns_explicit_original_fallback() {
    let source = "记下那部围绕亡灵节的动画";
    let invalid = serde_json::json!({"text":"那部围绕亡灵节的动画", "annotations":[
        {"description":"原文没有的情节", "name":"合成作品", "kind":"work"}]}).to_string();
    let valid = serde_json::json!({"text":"那部围绕亡灵节的动画", "annotations":[
        {"description":"那部围绕亡灵节的动画", "name":"合成作品", "kind":"work"}]}).to_string();
    for repaired in [&valid, &invalid] {
        let response = |s: &str| serde_json::json!({"choices":[{"message":{"content":s}}]}).to_string();
        let (base, requests, thread) = server(vec![("200 OK", response(&invalid)), ("200 OK", response(repaired))]);
        let config = ConnectionConfig { base, model: "synthetic".into(), protocol: "openai_chat".into() };
        let plan = vao_core::WritingPlan::resolve(source, "Notes", &[], &Profile::default());
        let request = models::WritingRequest { text: source, plan: &plan, dictionary: &[],
            translation: None, selection: None,
            options: vao_core::WritingOptions { semantic_completion: true } };
        let output = models::refine_writing(&config, "", &request).await.unwrap_or_else(|e| panic!("{e}"));
        assert!(output.first_issues.contains(&"annotation_description_not_preserved"));
        assert_eq!(output.generation_attempts, 2);
        assert!(output.used_original);
        assert_eq!(output.text, source);
        thread.join().unwrap();
        assert_eq!(requests.lock().unwrap().len(), 2);
    }
}


#[tokio::test]
async fn backup_budget_and_failed_repair_cannot_start_another_repair() {
    let text="明天寄材料。";let plan=vao_core::WritingPlan::resolve(text,"Notes",&[],&Profile::default());
    let request=models::WritingRequest{text,plan:&plan,dictionary:&[],translation:None,selection:None,options:Default::default()};
    let reply=r#"{"choices":[{"message":{"content":"寄材料。"}}]}"#.to_owned();
    let (base,_,thread)=server(vec![("200 OK",reply.clone())]);let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
    let out=models::refine_writing_limited(&config,"",&request,1).await.unwrap_or_else(|e|panic!("{e}"));assert!(out.used_original);assert_eq!(out.generation_attempts,1);assert!(!out.repair_attempted);thread.join().unwrap();
    let (base,_,thread)=server(vec![("200 OK",reply),("500 Internal Server Error",r#"{"error":{"code":"server_error"}}"#.into())]);
    let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
    let err=models::refine_writing(&config,"",&request).await.err().unwrap();assert!(!err.retryable);thread.join().unwrap();
}


#[tokio::test]
async fn book_off_repair_uses_original_and_never_restores_consumed_edit_instructions() {
    let text="周三给苏文发通知，前面的文改成文字的文。";
    let reply=|s:&str|serde_json::json!({"choices":[{"message":{"content":s}}]}).to_string();
    let (base,requests,thread)=server(vec![("200 OK",reply("给苏文发通知（《未口述作品》）。")),("200 OK",reply("周三给苏文发通知。"))]);
    let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
    let plan=vao_core::WritingPlan::resolve(text,"Notes",&[],&Profile::default());
    let request=models::WritingRequest{text,plan:&plan,dictionary:&[],translation:None,selection:None,options:Default::default()};
    let output=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{e}"));
    assert!(output.repair_attempted);assert!(!output.used_original);assert_eq!(output.generation_attempts,2);
    assert!(output.first_issues.contains(&"unrequested_work_title"));
    thread.join().unwrap();
    for raw in requests.lock().unwrap().iter() {
        let body:serde_json::Value=serde_json::from_str(raw.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert!(!body["messages"][0]["content"].as_str().unwrap().contains("引用后的全部说明"));
        let data:serde_json::Value=serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();assert_eq!(data["transcript"],text);
    }
}


#[tokio::test]
async fn completion_layout_matching_and_filler_cleanup_share_the_two_call_budget() {
    let source="记录，那个、那个，演员乙演的，那部独特旅行电影，下周四再看。";
    let body="记录，那个、那个，演员乙演的那部独特旅行电影，下周四再看。";
    let description="演员乙演的，那部独特旅行电影";
    let draft=serde_json::json!({"text":body,"annotations":[{"description":description,"name":"作品乙","kind":"work","mode":"replace"}]}).to_string();
    let review=serde_json::json!({"resolutions":[{"description":description,"names":["作品乙"],"kind":"work","unique":true,"identifying_only":true,"matches_all_clues":true}]}).to_string();
    let reply=|s:&str|serde_json::json!({"choices":[{"message":{"content":s}}]}).to_string();
    let (base,requests,thread)=server(vec![("200 OK",reply(&draft)),("200 OK",reply(&review))]);
    let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
    let plan=vao_core::WritingPlan::resolve(source,"Notes",&[],&Profile::default());
    let request=models::WritingRequest{text:source,plan:&plan,dictionary:&[],translation:None,selection:None,options:vao_core::WritingOptions{semantic_completion:true}};
    let result=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{e}"));
    assert_eq!(result.text,"记录，演员乙演的《作品乙》，下周四再看。");
    assert_eq!(result.generation_attempts,2);assert!(!result.used_original && !result.repair_attempted);
    thread.join().unwrap();assert_eq!(requests.lock().unwrap().len(),2);
}


#[tokio::test]
async fn completion_never_bypasses_review_when_only_one_generation_remains() {
    let source="记下那本独特故事的小说。";
    let content=serde_json::json!({"text":source,"annotations":[{"description":"那本独特故事的小说","name":"不能直接输出的候选","kind":"work","mode":"replace"}]}).to_string();
    let (base,requests,thread)=server(vec![("200 OK",serde_json::json!({"choices":[{"message":{"content":content}}]}).to_string())]);
    let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
    let plan=vao_core::WritingPlan::resolve(source,"Notes",&[],&Profile::default());
    let request=models::WritingRequest{text:source,plan:&plan,dictionary:&[],translation:None,selection:None,options:vao_core::WritingOptions{semantic_completion:true}};
    let result=models::refine_writing_limited(&config,"",&request,1).await.unwrap_or_else(|e|panic!("{e}"));
    assert_eq!(result.text,source);assert_eq!(result.completion_review,"budget_exhausted");
    assert_eq!(result.completion_applied,0);assert!(!result.used_original);
    thread.join().unwrap();assert_eq!(requests.lock().unwrap().len(),1);
}


#[tokio::test]
async fn completion_review_is_candidate_blind_and_fails_closed_without_losing_cleanup() {
    let source="帮我记作家甲的那本带整座城市移动的小说，周五买。";
    let description="那本带整座城市移动的小说";
    let body="帮我记作家甲的那本带整座城市移动的小说，周五买。";
    let first=serde_json::json!({"text":body,"annotations":[{"description":description,"name":"候选作品甲","kind":"work","mode":"replace"}]}).to_string();
    let reply=|text:&str|serde_json::json!({"choices":[{"message":{"content":text}}]}).to_string();
    for (name,unique,status,expected) in [
        ("候选作品甲",true,"200 OK","帮我记作家甲的《候选作品甲》，周五买。"),
        ("其他作品",true,"200 OK",body),
        ("候选作品甲",false,"200 OK",body),
        ("候选作品甲",true,"503 Service Unavailable",body),
    ] {
        let review=serde_json::json!({"resolutions":[{"description":description,"names":[name],"kind":"work","unique":unique,"identifying_only":true,"matches_all_clues":true}]}).to_string();
        let (base,requests,thread)=server(vec![("200 OK",reply(&first)),(status,reply(&review))]);
        let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
        let plan=vao_core::WritingPlan::resolve(source,"Notes",&[],&Profile::default());
        let request=models::WritingRequest{text:source,plan:&plan,dictionary:&[],translation:None,selection:None,options:vao_core::WritingOptions{semantic_completion:true}};
        let result=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{e}"));
        assert_eq!(result.text,expected);assert_eq!(result.generation_attempts,2);
        assert!(!result.used_original && !result.repair_attempted);
        thread.join().unwrap();let captured=requests.lock().unwrap();assert_eq!(captured.len(),2);
        let second:serde_json::Value=serde_json::from_str(captured[1].split_once("\r\n\r\n").unwrap().1).unwrap();
        assert!(!second.to_string().contains("候选作品甲"));
        assert!(!second.to_string().contains("不要复制的输入框内容"));
        let data:serde_json::Value=serde_json::from_str(second["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(data["transcript"],source);assert_eq!(data["writing"]["descriptions"][0],description);
    }
}


#[tokio::test]
async fn dictation_and_repair_send_only_the_spoken_source() {
    let (base,requests,thread)=server(vec![
        ("200 OK",r#"{"choices":[{"message":{"content":"发送材料。"}}]}"#.into()),
        ("200 OK",r#"{"choices":[{"message":{"content":"星期二发送材料。"}}]}"#.into())]);
    let text="星期二发送材料。";let plan=vao_core::WritingPlan::resolve(text,"Mail",&[],&Profile::default());
    let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
    let request=models::WritingRequest{text,plan:&plan,dictionary:&[],translation:None,selection:None,options:Default::default()};
    let output=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{}",e.message));assert!(output.repair_attempted);assert_eq!(output.text,text);
    thread.join().unwrap();for raw in requests.lock().unwrap().iter(){let body:serde_json::Value=serde_json::from_str(raw.split_once("\r\n\r\n").unwrap().1).unwrap();let content:serde_json::Value=serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();assert_eq!(content["transcript"],text);assert!(content.get("context").is_none());assert!(content.get("pid").is_none());}
}

#[tokio::test]
async fn entity_name_cannot_replace_explicit_product_identifiers() {
    let text="备忘那个制造WidgetX和DeviceY的公司。";
    let good="备忘那个制造WidgetX和DeviceY的公司（合成公司）。";
    let reply=|s:&str|serde_json::json!({"choices":[{"message":{"content":serde_json::json!({"text":s,"annotations":[]}).to_string()}}]}).to_string();
    let (base,_,thread)=server(vec![("200 OK",reply("合成公司。")),("200 OK",reply(good))]);
    let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
    let plan=vao_core::WritingPlan::resolve(text,"Notes",&[],&Profile::default());
    let request=models::WritingRequest{text,plan:&plan,dictionary:&[],translation:None,selection:None,options:vao_core::WritingOptions{semantic_completion:true}};
    let output=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{e}"));
    assert!(output.first_issues.contains(&"explicit_entity_identifier_missing"));assert!(!output.used_original);assert_eq!(output.text,good);thread.join().unwrap();
}


#[tokio::test]
async fn intent_outputs_survive_postprocessing_without_leaking_applied_spelling() {
    let message = "小明你好我最近换手机号码了电话是13812345678谢谢Jack前面的晓明是黄晓明的晓明";
    let expected = "晓明你好，我最近换手机号码了，电话是 13812345678，谢谢。\nJack";
    let books = "帮我记几本想看的书钱德勒那本漫长的告别石黑一雄写机器人的那本还有一个人在火星上种土豆的那本小说";
    let list = "1. 钱德勒的《漫长的告别》\n2. 石黑一雄写机器人的那本（《克拉拉与太阳》）\n3. 一个人在火星上种土豆的那本小说（《火星救援》）";
    for (source, generated, expected, completion) in [
        (message, expected.to_owned(), expected, false),
        (message, format!("{expected}。前面的晓明是黄晓明的晓明。"),
            "晓明你好，我最近换手机号码了，电话是 13812345678，谢谢。\nJack。", false),
        (books, list.to_owned(), list, true),
        ("陈老师您好请替我谢谢Nora帮忙", "陈老师您好，请替我谢谢Nora帮忙。".into(),
            "陈老师您好，请替我谢谢Nora帮忙。", false),
    ] {
        let generated = if completion { serde_json::json!({
            "text":"1. 钱德勒的《漫长的告别》\n2. 石黑一雄写机器人的那本\n3. 一个人在火星上种土豆的那本小说",
            "annotations":[
                {"description":"石黑一雄写机器人的那本","name":"克拉拉与太阳","kind":"work"},
                {"description":"一个人在火星上种土豆的那本小说","name":"火星救援","kind":"work"}]
        }).to_string() } else { generated };
        let reply = serde_json::json!({"choices":[{"message":{"content":generated}}]}).to_string();
        let mut replies = vec![("200 OK", reply)];
        if completion {
            let review = serde_json::json!({"resolutions":[
                {"description":"石黑一雄写机器人的那本","names":["克拉拉与太阳"],"kind":"work","unique":true,"identifying_only":false,"matches_all_clues":true},
                {"description":"一个人在火星上种土豆的那本小说","names":["火星救援"],"kind":"work","unique":true,"identifying_only":true,"matches_all_clues":true}]}).to_string();
            replies.push(("200 OK",serde_json::json!({"choices":[{"message":{"content":review}}]}).to_string()));
        }
        let (base, requests, thread) = server(replies);
        let config = ConnectionConfig { base, model: "synthetic".into(), protocol: "openai_chat".into() };
        let plan = vao_core::WritingPlan::resolve(source, "Notes", &[], &Profile::default());
        let request = models::WritingRequest { text: source, plan: &plan, dictionary: &[],
            translation: None, selection: None,
            options: vao_core::WritingOptions { semantic_completion: completion } };
        let result = models::refine_writing(&config, "", &request).await.unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(result.text, expected);
        assert!(!result.used_original);
        assert!(!result.repair_attempted);
        thread.join().unwrap();
        assert_eq!(requests.lock().unwrap().len(), if completion {2} else {1});
    }
}


#[tokio::test]
async fn legacy_and_special_operations_keep_completion_inactive() {
    let (base,requests,thread)=server(vec![("200 OK",r#"{"text":"合成文字。"}"#.into())]);
    let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"vao_refine".into()};
    let plan=vao_core::WritingPlan::resolve("合成文字。","Notes",&[],&Profile::default());
    let mut request=models::WritingRequest{text:"合成文字。",plan:&plan,dictionary:&[],translation:None,selection:None,options:vao_core::WritingOptions{semantic_completion:true}};
    let output=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{e}"));
    assert!(!output.semantic_effective&&!output.completion_effective&&output.completion_requested);
    thread.join().unwrap();{let captured=requests.lock().unwrap();let body:serde_json::Value=serde_json::from_str(captured[0].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body.as_object().unwrap().len(),2);assert!(body["preferences"].as_array().unwrap().iter().map(|v|v.as_str().unwrap().chars().count()).sum::<usize>()<=800);}
    let config=ConnectionConfig{base:crate::trial::BASE.into(),model:"any-alias".into(),protocol:"openai_chat".into()};
    assert!(models::semantic_supported(&config));
    for (translation,selection,responses) in [
        (Some("English"),None,vec![("200 OK",r#"{"choices":[{"message":{"content":"Synthetic text."}}]}"#.into())]),
        (None,Some("合成原文"),vec![("200 OK",r#"{"choices":[{"message":{"content":"{\"allowed\":true}"}}]}"#.into()),("200 OK",r#"{"choices":[{"message":{"content":"合成改文。"}}]}"#.into())]),
    ] {
        let (base,requests,thread)=server(responses);let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
        request.translation=translation;request.selection=selection;
        let out=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{e}"));assert!(!out.semantic_effective&&!out.completion_effective);
        thread.join().unwrap();let all=requests.lock().unwrap();let raw=all.last().unwrap();let body:serde_json::Value=serde_json::from_str(raw.split_once("\r\n\r\n").unwrap().1).unwrap();
        assert!(!body["messages"][0]["content"].as_str().unwrap().contains("智能补全"));
        let data:serde_json::Value=serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(data["operation"],if selection.is_some(){"selection_action"}else{"translation"});
    }
}


#[tokio::test]
async fn literal_spelling_evidence_reaches_only_custom_dictation_and_repairs_once() {
    let text="子期你好，明天见，前面的子琪是安琪的子琪。";
    let reply=|s:&str|serde_json::json!({"choices":[{"message":{"content":s}}]}).to_string();
    let (base,requests,thread)=server(vec![("200 OK",reply("子期你好，明天见。")),("200 OK",reply("子琪你好，明天见。"))]);
    let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
    let plan=vao_core::WritingPlan::resolve(text,"Messages",&[],&Profile::default());
    let request=models::WritingRequest{text,plan:&plan,dictionary:&[],translation:None,selection:None,options:Default::default()};
    let output=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{e}"));
    assert!(!output.used_original);assert!(output.repair_attempted);assert!(output.first_issues.contains(&"explicit_greeting_spelling_missing"));
    thread.join().unwrap();
    for raw in requests.lock().unwrap().iter() {
        let body:serde_json::Value=serde_json::from_str(raw.split_once("\r\n\r\n").unwrap().1).unwrap();
        let data:serde_json::Value=serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(data["transcript"],text);assert_eq!(data["spelling_evidence"][0]["expected_greeting_name"],"子琪");
        assert_eq!(data["spelling_evidence"][0]["specified_spelling"],"子琪");
    }
}


#[tokio::test]
async fn omitted_book_exclusion_is_repaired_or_retains_original() {
    let text="那本不是合成作品甲，我说的是另一本。";
    let reply=r#"{"choices":[{"message":{"content":"那本是《合成作品甲》，我说的是另一本。"}}]}"#.to_owned();
    let (base,_,thread)=server(vec![("200 OK",reply.clone()),("200 OK",reply)]);
    let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
    let plan=vao_core::WritingPlan::resolve(text,"Notes",&[],&Profile::default());
    let request=models::WritingRequest{text,plan:&plan,dictionary:&[],translation:None,selection:None,options:vao_core::WritingOptions{semantic_completion:true}};
    let output=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{e}"));
    assert!(!output.completion_effective);assert!(output.used_original);assert_eq!(output.text,text);
    assert!(output.final_issues.contains(&"entity_exclusion_missing"));thread.join().unwrap();
}


#[tokio::test]
async fn premature_title_uses_remaining_call_for_blind_review_not_repair() {
    let source="记那本独特航海小说，保留这段描述，在后面加上书名。周五再买。";
    let draft=serde_json::json!({"text":"记那本独特航海小说（《候选甲》）。周五再买。",
        "annotations":[{"description":"那本独特航海小说","name":"候选甲","kind":"work","mode":"annotate"}]}).to_string();
    let reply=|s:&str|serde_json::json!({"choices":[{"message":{"content":s}}]}).to_string();
    for (name,expected,applied) in [("候选甲","记那本独特航海小说（《候选甲》）。周五再买。",1),
        ("其他名称","记那本独特航海小说。周五再买。",0)] {
        let review=serde_json::json!({"resolutions":[{"description":"那本独特航海小说","names":[name],"kind":"work","unique":true,"identifying_only":true,"matches_all_clues":true}]}).to_string();
        let (base,requests,thread)=server(vec![("200 OK",reply(&draft)),("200 OK",reply(&review))]);
        let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
        let plan=vao_core::WritingPlan::resolve(source,"Notes",&[],&Profile::default());
        let request=models::WritingRequest{text:source,plan:&plan,dictionary:&[],translation:None,selection:None,options:vao_core::WritingOptions{semantic_completion:true}};
        let result=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{e}"));
        assert_eq!(result.text,expected);assert_eq!(result.generation_attempts,2);
        assert!(!result.repair_attempted && !result.used_original);assert_eq!(result.completion_applied,applied);
        thread.join().unwrap();let captured=requests.lock().unwrap();assert_eq!(captured.len(),2);
        assert!(!captured[1].contains("候选甲"));
    }
}

#[tokio::test]
async fn requested_books_are_inactive_for_exclusions_uncertainty_and_questions() {
    for text in ["记的那本不是《合成作品》，是另一本。","不确定作者，记不清了。","这本书叫什么？"] {
        let (base,requests,thread)=server(vec![("200 OK",serde_json::json!({"choices":[{"message":{"content":text}}]}).to_string())]);
        let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
        let plan=vao_core::WritingPlan::resolve(text,"Notes",&[],&Profile::default());
        let request=models::WritingRequest{text,plan:&plan,dictionary:&[],translation:None,selection:None,options:vao_core::WritingOptions{semantic_completion:true}};
        let output=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{e}"));
        assert!(output.completion_requested);assert!(!output.completion_effective);assert_eq!(output.text,text);
        thread.join().unwrap();let captured=requests.lock().unwrap();let body:serde_json::Value=serde_json::from_str(captured[0].split_once("\r\n\r\n").unwrap().1).unwrap();
        assert!(body["messages"][0]["content"].as_str().unwrap().contains("智能补全关闭"));
        assert!(!body["messages"][0]["content"].as_str().unwrap().contains("智能补全测试开启"));
    }
}


#[tokio::test]
async fn semantic_option_reaches_chat_as_versioned_data() {
    for enabled in [false,true] {
        let output=if enabled {serde_json::json!({"text":"保留书目描述。","annotations":[]}).to_string()} else {"保留书目描述。".into()};
        let (base,requests,thread)=server(vec![("200 OK",serde_json::json!({"choices":[{"message":{"content":output}}]}).to_string())]);
        let config=ConnectionConfig{base,model:"any-user-selected-model".into(),protocol:"openai_chat".into()};
        let text="保留书目描述。";
        let plan=vao_core::WritingPlan::resolve(text,"Notes",&[],&Profile::default());
        let request=models::WritingRequest{text,plan:&plan,dictionary:&[],translation:None,selection:None,options:vao_core::WritingOptions{semantic_completion:enabled}};
        let result=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{e}"));
        assert!(result.semantic_effective);assert_eq!(result.completion_effective,enabled);assert_eq!(result.generation_attempts,1);
        thread.join().unwrap();
        let captured=requests.lock().unwrap();let body:serde_json::Value=serde_json::from_str(captured[0].split_once("\r\n\r\n").unwrap().1).unwrap();
        let system=body["messages"][0]["content"].as_str().unwrap();
        assert_eq!(system.contains("智能补全测试开启"),enabled);
        assert_eq!(system.contains("智能补全关闭"),!enabled);
        let data:serde_json::Value=serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(data["writing"].as_object().unwrap().len(),9);
        assert_eq!(data["writing"]["policy_version"],vao_core::POLICY_VERSION);
        assert_eq!(data["writing"]["semantic_completion"],enabled);
        assert_eq!(data["writing"]["layout_source"],"default");
        assert_eq!(data["operation"],"dictation_cleanup");assert_eq!(data["transcript"],text);
        assert!(data["writing"].get("complete_book_titles").is_none());
    }
}


#[test]
fn semantic_setting_migration_preserves_unknown_fields_and_session_snapshot() {
    let path=std::env::temp_dir().join(format!("vao-semantic-settings-{}.sqlite3",std::process::id()));
    let db=Store::open(&path).unwrap();
    let original=serde_json::json!({"complete_book_titles":false,"llm":{"base":"https://example.com/v1","model":"original","protocol":"openai_chat"},"backup":{"base":"https://backup.example/v1","model":"original-backup","protocol":"openai_chat"},"dictionary":["SyntheticWord"],"auto_stop":true,"silence_seconds":7,"auto_input":false,"live_preview":true,"input_context":true,"expanded_context":true,"asr":{"base":"https://speech.example/v1","model":"unchanged-model","protocol":"openai_realtime"},"windows_return_to_origin":true,"windows_compatible_input":false,"future_platform":{"retained":42}});
    db.set("settings",&original).unwrap();
    let mut settings:Settings=db.get("settings");assert!(!settings.complete_book_titles);
    let frozen=settings.clone();settings.complete_book_titles=true;
    db.save_settings_preserving(&settings).unwrap();
    let updated:serde_json::Value=db.get("settings");
    for (key,value) in original.as_object().unwrap(){if !matches!(key.as_str(),"complete_book_titles"|"live_preview"|"input_context"|"expanded_context"|"asr"){assert_eq!(updated[key],*value,"{key}");}}
    assert!(updated.get("input_context").is_none() && updated.get("expanded_context").is_none() && updated.get("live_preview").is_none());
    assert_eq!(updated["asr"],serde_json::json!({"base":"https://speech.example/v1","model":"unchanged-model","protocol":"openai_audio"}));
    let visible=serde_json::to_value(&frozen).unwrap();
    assert!(visible.get("input_context").is_none() && visible.get("expanded_context").is_none() && visible.get("live_preview").is_none());
    assert_eq!(updated["complete_book_titles"],true);assert!(!frozen.complete_book_titles);
    let next:Settings=db.get("settings");assert!(next.complete_book_titles);
    // An old serde reader ignores the new field; its old typed serializer
    // may drop it on save. Do not promise lossless downgrade of new choices.
    #[derive(serde::Deserialize)]
    struct OldReader { llm:ConnectionConfig, dictionary:Vec<String>, auto_input:bool }
    let old:OldReader=serde_json::from_value(updated).unwrap();
    assert_eq!(old.llm.model,"original");assert_eq!(old.dictionary,vec!["SyntheticWord"]);assert!(!old.auto_input);
    drop(db);let _=std::fs::remove_file(path);
}


#[test]
fn smart_completion_defaults_on_and_respects_saved_opt_out() {
    assert!(Settings::default().complete_book_titles);
    let missing: Settings = serde_json::from_value(serde_json::json!({"auto_input": false})).unwrap();
    assert!(missing.complete_book_titles);
    assert!(!missing.auto_input);
    let opted_out: Settings = serde_json::from_value(serde_json::json!({"complete_book_titles": false})).unwrap();
    assert!(!opted_out.complete_book_titles);
    let roundtrip: Settings = serde_json::from_value(serde_json::to_value(opted_out).unwrap()).unwrap();
    assert!(!roundtrip.complete_book_titles);
}


#[tokio::test]
async fn spelling_reference_name_is_not_a_valid_substring_match() {
    let text="子期你好，前面的子琪是安琪的子琪。";
    let reply=|s:&str|serde_json::json!({"choices":[{"message":{"content":serde_json::json!({"text":s,"annotations":[]}).to_string()}}]}).to_string();
    let (base,_,thread)=server(vec![("200 OK",reply("安子琪你好。")),("200 OK",reply("子琪你好。"))]);
    let config=ConnectionConfig{base,model:"synthetic".into(),protocol:"openai_chat".into()};
    let plan=vao_core::WritingPlan::resolve(text,"Messages",&[],&Profile::default());
    let request=models::WritingRequest{text,plan:&plan,dictionary:&[],translation:None,selection:None,options:vao_core::WritingOptions{semantic_completion:true}};
    let output=models::refine_writing(&config,"",&request).await.unwrap_or_else(|e|panic!("{e}"));
    assert!(output.first_issues.contains(&"explicit_greeting_spelling_missing"));assert!(!output.used_original);assert_eq!(output.text,"子琪你好。");thread.join().unwrap();
}
