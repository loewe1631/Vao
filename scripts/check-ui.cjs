// UI-only browser checks with explicit IPC doubles. Does not certify native audio.
const {chromium}=require(process.env.VAO_PLAYWRIGHT_MODULE||'playwright');
const path=require('node:path'),fs=require('node:fs');
const root=path.resolve(__dirname,'..'),out=path.join(root,'artifacts/checks');fs.mkdirSync(out,{recursive:true});
const checks=[];function check(name,ok){checks.push({name,passed:!!ok});if(!ok)throw Error(name);}
(async()=>{const browser=await chromium.launch({headless:true});const page=await browser.newPage({viewport:{width:1080,height:780}});const errors=[];page.on('pageerror',e=>errors.push(String(e)));
await page.addInitScript(({version})=>{
 Object.defineProperty(window,'ipc',{value:{postMessage(){}},configurable:false});
 const settings={asr:{base:'',model:'',protocol:'openai_audio'},llm:{base:'',model:'',protocol:'openai_chat'},backup:{base:'',model:'',protocol:'openai_chat'},language:'auto',translation_language:'English',auto_input:true,dictionary:[],ui_language:'zh',theme:'system',trial_enabled:true,shortcuts:{record:'Control+Alt+Space',translate:'Control+Alt+KeyT',cancel:'Escape'}};
 const db={settings,profile:{revision:27,learned_revision:0,learning:true,personalization:true,rules:[],votes:[],undo:[]},history:[],version,permissions:{microphone:'not_determined',accessibility:'not_granted',input_monitoring:'not_granted',bundle:'app.vaovao.desktop'},trial:{provisioned:true,consented:true,enabled:true,ready:true,asr_source:'trial',llm_source:'trial'}};
 const state={session:{id:0,phase:'idle'},hotkeys:true,result:null,level:0,error:'',translating:false};window.fake={db,state,calls:[]};
 window.fake.listeners={};window.__TAURI__={event:{listen:async(name,fn)=>{window.fake.listeners[name]=fn;return ()=>{};}},core:{invoke:async(command,args={})=>{window.fake.calls.push({command,args});switch(command){case'import_dictionary':if(fake.dictionaryCancel)return {cancelled:true};if(fake.dictionaryFail)throw 'Invalid dictionary';db.settings.dictionary=[...new Set([...db.settings.dictionary,'Imported term'])];return {added:1,total:db.settings.dictionary.length};case'export_dictionary':if(fake.dictionaryCancel)return {cancelled:true};return {count:db.settings.dictionary.length};case'permissions':return structuredClone(db.permissions);case'request_permission':if(!fake.permissionDenied)db.permissions[args.kind]='granted';return structuredClone(db.permissions);case'enroll_trial':if(fake.enrollFail)throw 'HTTP 429';db.settings.trial_consent_version='2026-10-18';db.trial={...db.trial,provisioned:true,consented:true,ready:true};return {device:{expires:2000000000,daily_limit:200,used_today:0}};case'trial_status':if(window.fake.trialFail)throw 'HTTP 503';if(fake.unlimited)return {device:{unlimited:true,expires:253402300799,daily_limit:2147483647,used_today:5000}};return {device:{expires:2000000000,daily_limit:200,used_today:2},max_audio_seconds:120};case'test_trial':return '后天下午四点开会。';case'shortcut_capture':return true;case'bootstrap':return structuredClone(db);case'snapshot':return structuredClone(state);case'save_shortcuts':if(window.fake.rejectShortcut)throw 'Shortcut unavailable';db.settings.shortcuts=structuredClone(args.shortcuts);return {saved:true,active:!window.fake.pendingKeys};case'save_settings':db.settings=structuredClone(args.settings);return;case'fetch_models':return ['test-model','<img src=x onerror=alert(1)>'];case'test_asr':if(fake.asrDeferred)return await new Promise(resolve=>{fake.resolveAsr=resolve;});if(fake.asrFail)throw 'Realtime configuration rejected';return {input:'This is a speech recognition test.',output:'This is a speech recognition test.',latency_ms:456,model:args.config.model,protocol:fake.asrWrongProtocol?'openai_audio':args.config.protocol};case'test_llm':if(fake.testDeferred)return await new Promise((resolve,reject)=>{fake.resolveTest=resolve;fake.rejectTest=reject;});if(fake.testFail)throw 'HTTP 401';return {input:'明天开会，不对，后天开会。',output:fake.testOutput||'后天开会。',latency_ms:321,model:args.config.model,protocol:args.config.protocol};case'toggle_recording':state.session.phase=state.session.phase==='listening'?'processing':'listening';return;case'toggle_translation':if(state.session.phase!=='listening'){state.session.phase='listening';state.translating=true;}else state.translating=!state.translating;return state.translating;case'drag_voicebar':return;case'dismiss_answer':state.answer='';return;case'cancel':state.session.phase='cancelled';return;case'profile_action':db.profile[args.action]=args.value;return;case'feedback':return false;case'key_status':return {asr:false,llm:false,backup:false};default:return;}}}};
},JSON.parse(fs.readFileSync(path.join(root,'release.json'),'utf8')));
await page.goto('file://'+path.join(root,'ui/index.html'));await page.locator('h1').waitFor();
check('webview_ipc_global_is_preserved',await page.evaluate(()=>typeof window.ipc==='object'&&Object.getOwnPropertyDescriptor(window,'ipc').configurable===false));check('brand_is_vao',await page.locator('.brand strong').innerText()==='Vao');check('eight_navigation_entries',await page.locator('.nav button').count()===8);
check('permission_readiness_is_honest',await page.locator('#top-status').innerText().then(x=>x.includes('权限')));check('readiness_panel_has_actions',await page.locator('.readiness button').count()===2);
check('first_run_onboarding_visible',await page.locator('.onboarding').count()===1);
check('first_run_finish_waits_for_microphone',await page.locator('#finish-setup').isDisabled());
check('no_automatic_enrollment',await page.evaluate(()=>!fake.calls.some(x=>x.command==='enroll_trial')));
await page.evaluate(()=>{fake.db.trial={provisioned:false,consented:false,enabled:true,ready:false,asr_source:'trial',llm_source:'trial'};});
await page.locator('.nav [data-page="models"]').click();
check('enrollment_has_no_repeated_quota_paragraph',await page.locator('.trial-enrollment>p').count()===0);
check('trial_requires_explicit_consent',await page.locator('#enroll-trial').isDisabled()&&await page.locator('#test-trial').isDisabled());
await page.locator('#trial-consent').check();
await page.evaluate(()=>fake.enrollFail=true);await page.locator('#enroll-trial').click();
await page.waitForFunction(()=>document.querySelector('.toast')?.textContent.includes('429'));
check('enrollment_failure_does_not_mark_ready',await page.evaluate(()=>!fake.db.trial.consented&&!fake.db.trial.ready));
check('enrollment_retry_available',!(await page.locator('#enroll-trial').isDisabled()));
await page.evaluate(()=>fake.enrollFail=false);await page.locator('#enroll-trial').click();
await page.waitForFunction(()=>fake.db.trial.consented===true);
check('consented_enrollment_activates_trial',await page.locator('#enroll-trial').count()===0&&!(await page.locator('#test-trial').isDisabled()));
check('webview_receives_no_device_credential',await page.evaluate(()=>!JSON.stringify(fake.db).includes('vao_device_')));
await page.locator('.nav [data-page="home"]').click();
await page.screenshot({path:path.join(out,'home-zh.png')});
for(const language of ['zh','en']){
 if(language==='en')await page.locator('#lang').click();
 for(const width of [820,1080,1440]){await page.setViewportSize({width,height:780});let baseline;
  for(const name of ['home','guide','habits','history','dictionary','models','settings','about']){
   await page.locator(`.nav [data-page="${name}"]`).click();const geom=await page.locator('.shell').boundingBox();if(!baseline)baseline=geom;
   check(`fixed_window_${language}_${width}_${name}`,geom.height===780&&geom.width===width);
   check(`no_horizontal_overflow_${language}_${width}_${name}`,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
  }
 }
}
await page.setViewportSize({width:1080,height:780});await page.locator('.nav [data-page="models"]').click();
await page.locator('#asr-base').fill('http://127.0.0.1:9000/v1');await page.locator('[data-fetch="asr"]').click();
check('model_picker_has_discovered_models',await page.locator('#asr-model option').count()===2);check('model_ids_are_escaped',await page.locator('#asr-model img').count()===0);
await page.locator('#asr-model').selectOption('test-model');await page.locator('#asr-key').fill('synthetic-browser-key');await page.locator('[data-save-connection="asr"]').click();await page.waitForFunction(()=>window.fake.calls.some(x=>x.command==='save_key'));
check('key_not_in_settings',await page.evaluate(()=>!JSON.stringify(fake.db.settings).includes('synthetic-browser-key')));check('password_cleared_after_save',await page.locator('#asr-key').inputValue()==='');
// Speech diagnostics validate the selected protocol, without microphone input or fallback.
check('no_restore_trial_actions',await page.locator('[data-restore-trial]').count()===0);
await page.locator('[data-test="asr"]').click();
await page.waitForFunction(()=>document.querySelector('[data-connection-result="asr"] .success'));
check('asr_file_test_uses_selected_protocol',await page.evaluate(()=>fake.calls.filter(x=>x.command==='test_asr').at(-1).args.config.protocol==='openai_audio'));
check('asr_test_reports_phrase_and_latency',await page.locator('[data-connection-result="asr"]').innerText().then(x=>x.includes('456 ms')&&x.includes('This is a speech recognition test.')));
await page.locator('[data-connection-result="asr"]').scrollIntoViewIfNeeded();await page.screenshot({path:path.join(out,'asr-file-test-en.png')});
await page.locator('[data-slot="asr"] details').evaluate(e=>e.open=true);await page.locator('#asr-protocol').selectOption('openai_realtime');
check('asr_protocol_edit_clears_report',await page.locator('[data-connection-result="asr"]').innerText()==='');
await page.locator('[data-test="asr"]').click();await page.waitForFunction(()=>document.querySelector('[data-connection-result="asr"] .success'));
check('asr_realtime_test_uses_selected_protocol',await page.evaluate(()=>fake.calls.filter(x=>x.command==='test_asr').at(-1).args.config.protocol==='openai_realtime'));
check('asr_realtime_report_names_protocol',await page.locator('[data-connection-result="asr"]').innerText().then(x=>x.includes('OpenAI Realtime')));
await page.evaluate(()=>{fake.beforeAsrFailure=fake.calls.length;fake.asrFail=true;});await page.locator('[data-test="asr"]').click();
await page.waitForFunction(()=>document.querySelector('[data-connection-result="asr"] .failed'));
check('asr_realtime_failure_no_success',await page.locator('[data-connection-result="asr"] .success').count()===0);
check('asr_realtime_failure_no_file_fallback',await page.evaluate(()=>fake.calls.slice(fake.beforeAsrFailure).filter(x=>x.command==='test_asr').length===1&&fake.calls.slice(fake.beforeAsrFailure).filter(x=>x.command==='test_asr').every(x=>x.args.config.protocol==='openai_realtime')&&!fake.calls.some(x=>x.command==='toggle_recording')));
await page.locator('[data-connection-result="asr"]').scrollIntoViewIfNeeded();await page.screenshot({path:path.join(out,'asr-realtime-failure-en.png')});
await page.evaluate(()=>{fake.asrFail=false;fake.asrWrongProtocol=true;});await page.locator('[data-test="asr"]').click();
await page.waitForFunction(()=>document.querySelector('[data-connection-result="asr"] .failed'));
check('asr_wrong_protocol_never_success',await page.locator('[data-connection-result="asr"]').innerText().then(x=>x.includes('mismatch')));
await page.evaluate(()=>{fake.asrWrongProtocol=false;fake.asrDeferred=true;});await page.locator('[data-test="asr"]').click();await page.waitForFunction(()=>typeof fake.resolveAsr==='function');
await page.locator('#asr-protocol').selectOption('openai_audio');
await page.evaluate(()=>{fake.asrDeferred=false;fake.resolveAsr({input:'old',output:'old',latency_ms:456,protocol:'openai_realtime'});});await page.waitForTimeout(80);
check('asr_old_protocol_result_discarded',await page.locator('[data-connection-result="asr"]').innerText()==='');
// Pending diagnostics survive navigation without duplicate requests or a stuck button.
await page.evaluate(()=>{fake.asrDeferred=true;});await page.locator('[data-test="asr"]').click();
await page.waitForFunction(()=>typeof fake.resolveAsr==='function');
await page.locator('.nav [data-page="home"]').click();await page.locator('.nav [data-page="models"]').click();
check('asr_pending_navigation_prevents_duplicate',await page.locator('[data-test="asr"]').isDisabled());
await page.evaluate(()=>{fake.asrDeferred=false;fake.resolveAsr({input:'This is a speech recognition test.',output:'This is a speech recognition test.',latency_ms:456,protocol:'openai_audio'});});
await page.waitForFunction(()=>document.querySelector('[data-connection-result="asr"] .success'));
check('asr_completed_navigation_reenables_test',!(await page.locator('[data-test="asr"]').isDisabled()));
for(const language of ['zh','en']){
 if(await page.evaluate(()=>fake.db.settings.ui_language)!==language)await page.locator('#lang').click();
 for(const appearance of ['light','dark'])for(const width of [820,1080]){
  await page.emulateMedia({colorScheme:appearance});await page.setViewportSize({width,height:780});
  await page.locator('[data-connection-result="asr"]').scrollIntoViewIfNeeded();
  check(`asr_report_no_overflow_${language}_${appearance}_${width}`,await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
  await page.screenshot({path:path.join(out,`asr-result-${language}-${appearance}-${width}.png`)});
 }
}
await page.emulateMedia({colorScheme:'light'});await page.setViewportSize({width:1080,height:780});
// Clear only ASR's displayed report before the independent text-test assertions.
await page.locator('#asr-base').fill('http://127.0.0.1:9000/v1/');
// Persistent connection test reports, independent primary/backup, stale request protection.
await page.locator('#llm-base').fill('http://127.0.0.1:9998/v1');
await page.locator('[data-test="llm"]').click();
await page.waitForFunction(()=>document.querySelector('[data-connection-result="llm"]').textContent.includes('321 ms'));
check('connection_test_reports_latency',await page.locator('[data-connection-result="llm"]').innerText().then(x=>x.includes('321 ms')&&x.includes('Connection test successful')));
check('connection_test_reports_input_and_output',await page.locator('[data-connection-result="llm"] dd').allTextContents().then(x=>x.slice(-2).join('|')==='明天开会，不对，后天开会。|后天开会。'));
await page.locator('#backup-base').fill('http://127.0.0.1:9997/v1');await page.locator('[data-test="backup"]').click();
await page.waitForFunction(()=>document.querySelector('[data-connection-result="backup"] .success'));
check('connection_test_primary_and_backup_independent',await page.locator('.connection-test.success').count()===2);
await page.locator('[data-slot="llm"] details').evaluate(e=>e.open=true);await page.locator('#llm-protocol').selectOption('vao_refine');await page.locator('[data-test="llm"]').click();await page.waitForFunction(()=>document.querySelector('[data-connection-result="llm"] .success'));
check('text_diagnostic_uses_refine_protocol',await page.evaluate(()=>fake.calls.filter(x=>x.command==='test_llm').at(-1).args.config.protocol==='vao_refine'));
await page.locator('#llm-protocol').selectOption('openai_chat');
await page.locator('#llm-base').fill('http://127.0.0.1:9996/v1');
check('connection_edit_invalidates_only_its_report',await page.locator('[data-connection-result="llm"]').innerText()===''&&await page.locator('[data-connection-result="backup"] .success').count()===1);
await page.evaluate(()=>fake.testFail=true);await page.locator('[data-test="llm"]').click();await page.waitForFunction(()=>document.querySelector('[data-connection-result="llm"]').textContent.includes('401'));
check('connection_failure_has_no_success_or_latency',await page.locator('[data-connection-result="llm"]').innerText().then(x=>x.includes('Connection test failed')&&!x.includes('321 ms')));
await page.evaluate(()=>{fake.testFail=false;fake.testDeferred=true;});await page.locator('[data-test="llm"]').click();
await page.waitForFunction(()=>typeof fake.resolveTest==='function');
check('connection_test_has_pending_state',await page.locator('[data-connection-result="llm"]').innerText().then(x=>x.includes('Testing')));
await page.locator('#llm-key').fill('synthetic-replaced-key');await page.evaluate(()=>{fake.resolveTest({input:'old',output:'old',latency_ms:999});fake.testDeferred=false;});await page.waitForTimeout(80);
check('connection_stale_response_discarded',await page.locator('[data-connection-result="llm"]').innerText()==='');
await page.evaluate(()=>fake.testOutput='<img src=x onerror=alert(1)>');await page.locator('[data-test="llm"]').click();await page.waitForFunction(()=>document.querySelector('[data-connection-result="llm"] .success'));
check('connection_response_escaped',await page.locator('[data-connection-result="llm"] img').count()===0&&await page.locator('[data-connection-result="llm"]').innerText().then(x=>x.includes('<img')));
await page.locator('[data-save-connection="llm"]').click();await page.locator('[data-test="llm"]').click();await page.waitForFunction(()=>document.querySelector('[data-connection-result="llm"] .success'));
await page.locator('#lang').click();
check('connection_report_localizes_zh',await page.locator('[data-connection-result="llm"]').innerText().then(x=>x.includes('测试连接成功')&&x.includes('延迟')&&x.includes('测试原文')));
await page.screenshot({path:path.join(out,'connection-test-zh.png')});await page.locator('#lang').click();
await page.locator('.nav [data-page="settings"]').click();check('translation_shortcut_visible',await page.locator('kbd').allTextContents().then(v=>v.includes('⌃ ⌥ T')));check('automatic_insertion_default',await page.locator('#auto-input').getAttribute('aria-checked')==='true');await page.locator('#auto-input').click();check('preview_can_be_selected',await page.evaluate(()=>fake.db.settings.auto_input===false));await page.locator('#auto-input').click();check('auto_input_can_be_restored',await page.evaluate(()=>fake.db.settings.auto_input===true));
check('preview_opt_in_default',await page.locator('#live-preview').getAttribute('aria-checked')==='false');
check('context_opt_in_default',await page.locator('#input-context').getAttribute('aria-checked')==='false');
check('expanded_context_default_off',await page.locator('#expanded-context').getAttribute('aria-checked')==='false');await page.locator('#expanded-context').click();check('expanded_context_requires_input_context',await page.evaluate(()=>fake.db.settings.expanded_context!==true));
await page.locator('#live-preview').click();check('preview_setting_saved',await page.evaluate(()=>fake.db.settings.live_preview===true));
await page.locator('#input-context').click();check('context_setting_saved',await page.evaluate(()=>fake.db.settings.input_context===true));
await page.locator('#expanded-context').click();check('expanded_context_saved',await page.evaluate(()=>fake.db.settings.expanded_context===true));check('enhancement_details_initially_collapsed',await page.locator('.enhancement-details').evaluate(e=>!e.open));
check('short_hints_disclose_active_data_use',await page.locator('.enhancements .setting p').allTextContents().then(p=>p[0].includes('during recording')&&p[1].includes('text model')&&p[2].includes('Requires')));
await page.locator('.enhancement-details summary').click();check('expanded_context_discloses_visible_text_and_domain',await page.locator('.enhancements').innerText().then(x=>x.includes('1,200')&&x.includes('domain')));
check('enhancements_disclose_sending_and_requests',await page.locator('.enhancements').innerText().then(t=>t.includes('during recording')&&t.includes('500')));
await page.screenshot({path:path.join(out,'enhancements-settings-en.png')});await page.locator('.enhancement-details summary').click();
await page.locator('.nav [data-page="guide"]').click();await page.locator('[data-lesson="translate"]').click();await page.locator('#demo-play').click();await page.waitForTimeout(2800);check('guide_animation_advances',await page.locator('#demo-progress').innerText()!=='1 / 7');check('guide_does_not_record',await page.evaluate(()=>!fake.calls.some(c=>c.command==='toggle_recording')));
await page.locator('#theme').click();await page.locator('#theme').click();await page.screenshot({path:path.join(out,'guide-en-dark.png')});
await page.locator('.nav [data-page="home"]').click();await page.evaluate(()=>{fake.state.session={id:1,phase:'ready'};fake.state.result={id:1,created:Date.now()/1000,scene:'chat',layout:'paragraph',revision:0,original:'合成原文',text:'<script>unsafe</script>合成结果',risks:[],delivery:'preview',feedback:false};});await page.waitForFunction(()=>document.querySelector('#result-content')?.textContent.includes('合成结果'));
check('output_escaped',await page.locator('#result-content script').count()===0);check('explicit_feedback_controls',await page.locator('[data-feedback]').count()===3);await page.locator('[data-feedback="once"]').click();check('one_time_feedback_not_remembered',await page.evaluate(()=>fake.calls.filter(c=>c.command==='feedback').at(-1).args.remember===false));

for(const name of ['models','settings','guide']){
 await page.locator(`.nav [data-page="${name}"]`).click();
 await page.locator('.scroll').evaluate(e=>e.scrollTop=0);
 await page.mouse.move(750,360);await page.mouse.wheel(0,600);
 await page.waitForFunction(()=>document.querySelector('.scroll').scrollTop>0);
 check('mouse_wheel_scrolls_'+name,await page.locator('.scroll').evaluate(e=>e.scrollTop>0));
 check('toolbar_stays_visible_'+name,await page.locator('.toolbar').boundingBox().then(b=>b.y===0));
 await page.locator('.scroll').evaluate(e=>e.scrollTop=e.scrollHeight);
 check('bottom_reachable_'+name,await page.locator('.scroll').evaluate(e=>Math.abs(e.scrollTop+e.clientHeight-e.scrollHeight)<2));
}
await page.locator('.nav [data-page="settings"]').click();await page.locator('[data-edit-shortcut="record"]').click();
await page.locator('#capture-shortcut').click();await page.keyboard.press('Control+Shift+KeyR');await page.locator('#save-shortcut').click();
await page.waitForFunction(()=>fake.db.settings.shortcuts.record==='Control+Shift+KeyR');
check('custom_shortcut_saved',await page.locator('[data-shortcut-label="record"]').innerText()==='⌃ ⇧ R');
await page.locator('[data-edit-shortcut="translate"]').click();await page.locator('#capture-shortcut').click();
await page.keyboard.down('Meta');
check('command_modifier_shows_live_feedback',await page.locator('#capture-status').innerText().then(x=>x.includes('⌘')&&/继续|press another/.test(x)));
check('command_alone_does_not_complete_capture',await page.locator('#capture-shortcut').innerText().then(x=>/请按键|Press your keys/.test(x)));
await page.keyboard.press('Shift+KeyT');await page.keyboard.up('Meta');
check('command_chord_is_captured',await page.locator('#shortcut-input').inputValue()==='Shift+Super+KeyT');
await page.locator('#save-shortcut').click();await page.waitForFunction(()=>fake.db.settings.shortcuts.translate==='Shift+Super+KeyT');
check('command_combination_is_saved',await page.locator('[data-shortcut-label="translate"]').innerText().then(x=>x.includes('⌘')&&x.includes('⇧')&&x.includes('T')));
// Restore the baseline used by later conflict checks.
await page.locator('[data-edit-shortcut="translate"]').click();await page.locator('#shortcut-input').fill('Control+Alt+KeyT');await page.locator('#save-shortcut').click();
await page.locator('.nav [data-page="home"]').click();check('home_uses_custom_shortcut',await page.locator('.hero-actions kbd').innerText()==='⌃ ⇧ R');
await page.locator('.nav [data-page="guide"]').click();check('guide_uses_custom_shortcut',(await page.locator('kbd').allTextContents()).includes('⌃ ⇧ R'));
await page.locator('.nav [data-page="settings"]').click();await page.locator('[data-edit-shortcut="translate"]').click();await page.locator('#shortcut-input').fill('Control+Alt+KeyY');await page.evaluate(()=>fake.rejectShortcut=true);await page.locator('#save-shortcut').click();await page.waitForFunction(()=>document.querySelector('.toast')?.textContent.includes('Shortcut unavailable'));check('conflict_does_not_change_settings',await page.evaluate(()=>fake.db.settings.shortcuts.translate==='Control+Alt+KeyT'));check('failed_editor_stays_open',await page.locator('#shortcut-input').inputValue()==='Control+Alt+KeyY');await page.evaluate(()=>fake.rejectShortcut=false);await page.locator('#cancel-shortcut').click();
await page.locator('[data-edit-shortcut="cancel"]').click();await page.locator('#shortcut-input').fill('F8');await page.locator('#save-shortcut').click();await page.waitForFunction(()=>fake.db.settings.shortcuts.cancel==='F8');check('cancel_key_is_editable',await page.locator('[data-shortcut-label="cancel"]').innerText()==='F8');
await page.locator('#reset-shortcuts').click();await page.waitForFunction(()=>fake.db.settings.shortcuts.record==='Control+Alt+Space');check('restore_defaults',await page.evaluate(()=>fake.db.settings.shortcuts.cancel==='Escape'));

await page.locator('[data-edit-shortcut="record"]').click();await page.locator('#capture-shortcut').click();await page.evaluate(()=>fake.listeners['shortcut-captured']({payload:'Fn'}));check('fn_capture_received',await page.locator('#shortcut-input').inputValue()==='Fn');await page.locator('#save-shortcut').click();await page.waitForFunction(()=>fake.db.settings.shortcuts.record==='Fn');
await page.locator('[data-edit-shortcut="translate"]').click();await page.locator('#shortcut-input').fill('Fn+Space');await page.locator('#save-shortcut').click();await page.waitForFunction(()=>fake.db.settings.shortcuts.translate==='Fn+Space');check('fn_chord_manually_saved',await page.locator('[data-shortcut-label="translate"]').innerText()==='Fn Space');
await page.locator('[data-edit-shortcut="record"]').click();await page.screenshot({path:path.join(out,'settings-shortcut-entry.png')});
await page.locator('.nav [data-page="home"]').click();check('chrome_not_selectable',await page.locator('h1').evaluate(e=>getComputedStyle(e).userSelect==='none'));check('chrome_context_menu_suppressed',await page.locator('h1').evaluate(e=>!e.dispatchEvent(new MouseEvent('contextmenu',{bubbles:true,cancelable:true}))));check('result_stays_selectable',await page.locator('.output').first().evaluate(e=>getComputedStyle(e).userSelect==='text'));
await page.locator('.nav [data-page="dictionary"]').click();check('dictionary_instructions_present',await page.locator('.dictionary-help li').count()===3);check('dictionary_example_is_not_an_entry',await page.locator('#dictionary').inputValue()==='');check('editing_context_menu_retained',await page.locator('#dictionary').evaluate(e=>e.dispatchEvent(new MouseEvent('contextmenu',{bubbles:true,cancelable:true}))));await page.locator('#dictionary').fill('Vao\nAcme Studio\nVao');check('dictionary_count_deduplicates',await page.locator('#dictionary-count').innerText()==='2');await page.locator('#save-dictionary').click();check('dictionary_saves_terms_only',await page.evaluate(()=>fake.db.settings.dictionary.length===2));await page.screenshot({path:path.join(out,'dictionary-alpha3.png')});
check('dictionary_transfer_buttons_present',await page.locator('#import-dictionary').count()===1&&await page.locator('#export-dictionary').count()===1);
await page.locator('#dictionary').fill('Unsaved term');await page.locator('#import-dictionary').click();check('unsaved_dictionary_not_discarded',await page.evaluate(()=>!fake.calls.some(c=>c.command==='import_dictionary'))&&await page.locator('#dictionary').inputValue()==='Unsaved term');
await page.locator('#dictionary').fill('Vao\nAcme Studio');await page.evaluate(()=>fake.dictionaryCancel=true);await page.locator('#import-dictionary').click();check('cancel_import_preserves_dictionary',await page.locator('#dictionary').inputValue()==='Vao\nAcme Studio');
await page.evaluate(()=>{fake.dictionaryCancel=false;fake.dictionaryFail=true;});await page.locator('#import-dictionary').click();await page.waitForFunction(()=>document.querySelector('.toast')?.textContent.includes('Invalid dictionary'));check('invalid_import_preserves_dictionary',await page.evaluate(()=>fake.db.settings.dictionary.length===2));
await page.evaluate(()=>fake.dictionaryFail=false);await page.locator('#import-dictionary').click();await page.waitForFunction(()=>document.querySelector('#dictionary').value.includes('Imported term'));check('successful_import_refreshes_terms',await page.evaluate(()=>fake.db.settings.dictionary.length===3));
await page.locator('#export-dictionary').click();check('export_has_no_settings_or_keys_payload',await page.evaluate(()=>JSON.stringify(fake.calls.filter(c=>c.command==='export_dictionary').at(-1).args)==='{}'));
await page.screenshot({path:path.join(out,'dictionary-transfer-en.png')});
await page.locator('.nav [data-page="models"]').click();await page.locator('[data-slot="asr"] details').evaluate(e=>e.open=true);await page.locator('#asr-protocol').selectOption('openai_realtime');await page.locator('[data-save-connection="asr"]').click();check('realtime_protocol_user_selected_and_saved',await page.evaluate(()=>fake.db.settings.asr.protocol==='openai_realtime'));
await page.locator('#asr-protocol').selectOption('openai_audio');await page.locator('[data-save-connection="asr"]').click();check('file_protocol_can_be_restored',await page.evaluate(()=>fake.db.settings.asr.protocol==='openai_audio'));
await page.locator('[data-slot="llm"] details').evaluate(e=>e.open=true);
await page.locator('[data-slot="backup"] details').evaluate(e=>e.open=true);
await page.locator('#llm-protocol').selectOption('vao_refine');
check('legacy_refine_guidance_is_actionable',await page.locator('#llm-protocol-hint').innerText().then(x=>x.includes('/refine')&&x.includes('OpenAI Chat Completions')&&x.includes('No translation')));
check('protocol_hint_does_not_affect_backup_or_save_implicitly',await page.locator('#backup-protocol-hint').innerText().then(x=>x.includes('/chat/completions'))&&await page.evaluate(()=>fake.db.settings.llm.protocol==='openai_chat'));
await page.locator('[data-test="llm"]').click();await page.waitForFunction(()=>document.querySelector('[data-connection-result="llm"] .success'));
check('test_receives_explicit_legacy_protocol',await page.evaluate(()=>fake.calls.filter(c=>c.command==='test_llm').at(-1).args.config.protocol==='vao_refine'));
await page.locator('#llm-protocol').selectOption('openai_chat');
check('protocol_switch_invalidates_old_test',await page.locator('[data-connection-result="llm"]').innerText()==='');
check('chat_guidance_restored_in_place',await page.locator('#llm-protocol-hint').innerText().then(x=>x.includes('/chat/completions')));
await page.locator('#backup-protocol').selectOption('vao_refine');await page.locator('[data-save-connection="backup"]').click();
await page.locator('.nav [data-page="models"]').click();await page.locator('[data-slot="backup"] details').evaluate(e=>e.open=true);
check('saved_legacy_connection_keeps_value_and_guidance',await page.locator('#backup-protocol').inputValue()==='vao_refine'&&await page.locator('#backup-protocol-hint').innerText().then(x=>x.includes('/refine')));
await page.locator('#backup-protocol').selectOption('openai_chat');await page.locator('[data-save-connection="backup"]').click();
// Visual QA captures use synthetic state only; cover both languages, themes and minimum window width.
for(const lang of ['en','zh']){
 if(await page.locator('html').getAttribute('lang')!==(lang==='en'?'en':'zh-CN'))await page.locator('#lang').click();
 for(const appearance of ['light','dark']){
  while(await page.locator('html').getAttribute('data-theme')!==appearance)await page.locator('#theme').click();
  for(const width of [820,1080]){
   await page.setViewportSize({width,height:780});await page.locator('.nav [data-page="dictionary"]').click();
   const buttons=await page.locator('.dictionary-actions button').evaluateAll(es=>es.map(e=>{const r=e.getBoundingClientRect();return {x:r.x,y:r.y,right:r.right,bottom:r.bottom};}));
   check(`dictionary_buttons_separated_${lang}_${appearance}_${width}`,buttons.every((a,i)=>buttons.slice(i+1).every(b=>a.right+8<=b.x||b.right+8<=a.x||a.bottom+8<=b.y||b.bottom+8<=a.y)));
   check(`dictionary_card_contains_actions_${lang}_${appearance}_${width}`,await page.locator('.dictionary-actions').evaluate(e=>e.scrollWidth<=e.clientWidth));
   await page.locator('.scroll').evaluate(e=>e.scrollTop=e.scrollHeight);await page.evaluate(()=>document.querySelector('.toast')?.remove());await page.screenshot({path:path.join(out,`dictionary-${lang}-${appearance}-${width}.png`)});
   await page.locator('.nav [data-page="settings"]').click();await page.locator('.enhancements').scrollIntoViewIfNeeded();await page.screenshot({path:path.join(out,`enhancements-${lang}-${appearance}-${width}.png`)});
  }
 }
}
await page.setViewportSize({width:1080,height:780});await page.locator('.nav [data-page="models"]').click();
await page.locator('[data-slot="llm"] details').evaluate(e=>e.open=true);await page.locator('#llm-protocol').selectOption('vao_refine');await page.locator('#llm-protocol-hint').scrollIntoViewIfNeeded();await page.screenshot({path:path.join(out,'refine-guidance-zh.png')});
await page.locator('#llm-protocol').selectOption('openai_chat');
await page.locator('#lang').click(); // Restore English for the existing checks.

await page.evaluate(()=>fake.unlimited=true);await page.locator('#check-trial').click();await page.waitForFunction(()=>document.querySelector('#trial-status').textContent.includes('No expiry'));check('unlimited_service_has_no_misleading_legacy_numbers',await page.locator('#trial-status').innerText().then(x=>!x.includes('2147483647')&&!x.includes('9999')));
await page.locator('.nav [data-page="habits"]').click();const badge=await page.locator('.card .badge').innerText();check('legacy_switch_count_not_shown_as_learning',badge.endsWith('0'));await page.locator('#learning').click();await page.locator('#learning').click();await page.locator('#personalization').click();await page.locator('#personalization').click();check('toggles_do_not_change_visible_preference_version',await page.locator('.card .badge').innerText()===badge);
await page.locator('.nav [data-page="guide"]').click();await page.locator('[data-lesson="dictate"]').click();check('dictation_has_five_visible_steps',await page.locator('[data-demo-step]').count()===5);await page.locator('[data-demo-step="1"]').click();check('step_click_shows_input_and_listening',await page.locator('.demo-field').count()===1&&await page.locator('.demo .voicebar').innerText().then(x=>x.includes('Listening')));await page.locator('#demo-next').click();check('next_step_is_clickable',await page.locator('#demo-progress').innerText()==='3 / 5');await page.locator('#demo-prev').click();check('previous_step_is_clickable',await page.locator('#demo-progress').innerText()==='2 / 5');await page.locator('#demo-play').click();await page.locator('#demo-play').click();const paused=await page.locator('#demo-progress').innerText();await page.waitForTimeout(2800);check('pause_holds_step',await page.locator('#demo-progress').innerText()===paused);
await page.locator('[data-lesson="selection"]').click();await page.locator('[data-demo-step="0"]').click();check('selection_is_visually_highlighted',await page.locator('.demo-field mark').count()===1);
await page.locator('[data-lesson="translate"]').click();check('translation_has_seven_visible_steps',await page.locator('[data-demo-step]').count()===7);await page.locator('[data-demo-step="4"]').click();check('translation_off_keeps_listening',await page.locator('#demo').innerText().then(x=>x.includes('Translation OFF')&&x.includes('Listening')));await page.screenshot({path:path.join(out,'guide-alpha3-en.png')});await page.locator('#lang').click();await page.screenshot({path:path.join(out,'guide-alpha3-zh.png')});
await page.screenshot({path:path.join(out,'settings-guide.png')});

await page.locator('.nav [data-page="settings"]').click();
check('three_permission_states',await page.locator('[data-permission-status]').count()===3);
check('permission_not_inferred_from_old_app',await page.locator('[data-permission-status="microphone"]').innerText()==='待授权');
await page.locator('[data-request-permission="microphone"]').click();await page.waitForFunction(()=>document.querySelector('[data-permission-status="microphone"]')?.textContent==='已授权');
check('request_updates_real_state',await page.evaluate(()=>fake.calls.some(c=>c.command==='request_permission'&&c.args.kind==='microphone')));
await page.evaluate(()=>fake.db.permissions.accessibility='granted');await page.locator('#refresh-permissions').click();
await page.waitForFunction(()=>document.querySelector('[data-permission-status="accessibility"]')?.textContent==='已授权');
check('refresh_permission_status',await page.locator('[data-permission-status="accessibility"]').innerText()==='已授权');
await page.screenshot({path:path.join(out,'settings-permissions.png')});
await page.locator('.nav [data-page="models"]').click();
check('trial_discloses_remote_audio',await page.locator('.trial-card').innerText().then(x=>x.includes('音频')&&x.includes('体验服务器')));
check('trial_not_in_custom_slots',await page.locator('.model-card').count()===3&&await page.locator('.trial-card input').count()===0);
await page.evaluate(()=>fake.unlimited=false);await page.locator('#check-trial').click();await page.waitForFunction(()=>document.querySelector('#trial-status')?.textContent.includes('198 / 200'));
check('trial_live_quota_visible',await page.locator('#trial-status').innerText().then(x=>x.includes('198 / 200')));
await page.evaluate(()=>fake.trialFail=true);await page.locator('#check-trial').click();await page.waitForFunction(()=>document.querySelector('#trial-status')?.textContent.includes('503'));
check('failed_service_check_not_green',await page.locator('#trial-status .granted').count()===0);
await page.evaluate(()=>fake.trialFail=false);await page.locator('#check-trial').click();
await page.locator('#trial-enabled').click();check('trial_disable_persists',await page.evaluate(()=>fake.db.settings.trial_enabled===false));
check('disabled_trial_not_testable',await page.locator('#test-trial').isDisabled());await page.locator('#trial-enabled').click();
await page.locator('[data-slot="asr"] details').evaluate(e=>e.open=true);await page.locator('#asr-base').fill('');await page.locator('#asr-manual').fill('');await page.locator('[data-save-connection="asr"]').click();await page.waitForFunction(()=>fake.db.settings.asr.base==='');
check('manually_clearing_connection_keeps_shortcuts',await page.evaluate(()=>fake.db.settings.asr.base===''&&fake.db.settings.shortcuts.record==='Fn'));
await page.locator('#backup-base').fill('http://127.0.0.1:9997/v1');await page.locator('[data-save-connection="backup"]').click();await page.locator('.nav [data-page="models"]').click();
page.once('dialog',d=>d.accept());await page.locator('[data-clear-backup]').click();
check('disable_backup_only_clears_backup',await page.evaluate(()=>fake.db.settings.backup.base===''&&fake.db.settings.llm.base!==''));
await page.locator('.scroll').evaluate(e=>e.scrollTop=0);await page.screenshot({path:path.join(out,'models-routes.png')});
await page.locator('.nav [data-page="home"]').click();await page.screenshot({path:path.join(out,'home-readiness.png')});

await page.locator('.nav [data-page="settings"]').click();await page.locator('[data-edit-shortcut="record"]').click();
await page.locator('#capture-shortcut').click();
await page.waitForFunction(()=>document.querySelector('#capture-status')?.textContent.includes('无需先授权'));
check('local_capture_does_not_require_global_permission',await page.locator('#capture-status').innerText().then(x=>x.includes('无需先授权')));
await page.evaluate(()=>fake.listeners['shortcut-captured']({payload:'Fn'}));await page.evaluate(()=>fake.pendingKeys=true);
await page.locator('#save-shortcut').click();await page.waitForFunction(()=>document.querySelector('.toast')?.textContent.includes('授权生效后'));
check('pending_shortcut_not_claimed_active',await page.locator('.toast').innerText().then(x=>x.includes('已保存')&&x.includes('授权')));
await page.locator('.permission-repair summary').click();check('stale_permission_repair_actions_visible',await page.locator('#repair-accessibility').isVisible()&&await page.locator('#reveal-application').isVisible());
page.once('dialog',d=>d.dismiss());await page.locator('#repair-accessibility').click();check('repair_not_run_when_cancelled',await page.evaluate(()=>!fake.calls.some(c=>c.command==='repair_permission')));
check('both_permission_repair_actions_present',await page.locator('[data-repair-permission]').count()===2);
page.once('dialog',d=>d.dismiss());await page.locator('#repair-input-monitoring').click();check('input_reset_requires_confirmation',await page.evaluate(()=>!fake.calls.some(c=>c.command==='repair_permission')));
page.once('dialog',d=>d.dismiss());await page.locator('#restart-permissions').click();check('restart_requires_confirmation',await page.evaluate(()=>!fake.calls.some(c=>c.command==='restart_for_permissions')));
await page.evaluate(()=>{fake.permissionDenied=true;fake.db.permissions.input_monitoring='not_granted';});await page.locator('#refresh-permissions').click();await page.locator('[data-request-permission="input_monitoring"]').click();
await page.waitForFunction(()=>document.querySelector('#permission-request-help')?.textContent.includes('macOS'));
check('denied_request_not_claimed_granted',await page.locator('[data-permission-status="input_monitoring"]').innerText().then(x=>x.includes('未授权')));
check('missing_permission_opens_matching_settings',await page.evaluate(()=>fake.calls.some(c=>c.command==='open_permission'&&c.args.kind==='input_monitoring')));
check('manual_add_and_restart_are_explained',await page.locator('#input-monitoring-help').innerText().then(x=>x.includes('＋')&&x.includes('应用程序 → Vao')&&x.includes('重启 Vao')));
await page.evaluate(()=>{fake.permissionDenied=false;fake.db.permissions.accessibility='not_granted';});await page.locator('#refresh-permissions').click();await page.waitForFunction(()=>document.querySelector('#top-status').textContent.includes('跨应用'));
check('recording_ready_does_not_imply_cross_app_access',await page.locator('#top-status').innerText().then(x=>x.includes('跨应用权限待完成')));
await page.locator('[data-request-permission="accessibility"]').click();await page.waitForFunction(()=>document.querySelector('[data-permission-status="accessibility"]').textContent==='已授权');
await page.locator('#permissions-card').scrollIntoViewIfNeeded();await page.screenshot({path:path.join(out,'permissions-beta2.png')});

await page.locator('.nav [data-page="settings"]').click();await page.evaluate(()=>{fake.db.settings.auto_input=false;fake.state.session={id:8,phase:'processing'};fake.state.result={id:0,created:Date.now()/1000,scene:'chat',layout:'paragraph',revision:0,original:'原始识别',text:'原始识别',risks:['unrefined'],delivery:'preview',feedback:false};});
await page.waitForTimeout(1200);
await page.evaluate(()=>{fake.state.session.phase='ready';fake.state.result={...fake.state.result,id:8,text:'这次结果必须立即看到。',risks:[]};});
await page.waitForFunction(()=>document.querySelector('#result-card .output')?.textContent.includes('这次结果'));
check('completion_navigates_to_visible_result',await page.locator('#result-card .output').boundingBox().then(b=>b.y>=60&&b.y+b.height<=780));
check('preview_explains_no_automatic_insertion',await page.locator('.preview-explanation').innerText().then(x=>x.includes('不会自动填入')));
await page.locator('[data-enable-auto-input]').click();check('auto_insertion_enabled_only_on_explicit_action',await page.evaluate(()=>fake.db.settings.auto_input===true));
await page.evaluate(()=>{fake.state.result.risks=['numbers_require_review','protected_quote_changed','constraint_requires_review'];fake.listeners['session-updated']();});
await page.waitForTimeout(1200);
check('fidelity_diagnostics_do_not_show_forced_review_banner',await page.locator('#result-card .notice.warn').count()===0);
check('preview_does_not_claim_content_review_is_required',!(await page.locator('.preview-explanation').innerText()).includes('复核'));
await page.evaluate(()=>{fake.state.result.id=88;fake.state.result.risks=['audio_interrupted'];fake.listeners['session-updated']();});
await page.waitForFunction(()=>document.querySelector('#result-card .recording-warning')?.textContent.includes('录音中断'));
check('partial_audio_is_explicitly_labelled',await page.locator('.recording-warning').isVisible());


await page.locator('.nav [data-page="settings"]').click();await page.evaluate(()=>{fake.state.session={id:9,phase:'listening'};fake.state.result=null;fake.state.error='';});await page.waitForTimeout(1100);
await page.evaluate(()=>{fake.state.session.phase='failed';fake.state.error='语音服务暂不可用（HTTP 503）';});
await page.waitForFunction(()=>document.querySelector('.session-error')?.textContent.includes('HTTP 503'));
check('asr_failure_without_result_is_visible',await page.locator('.session-error').isVisible());
await page.waitForTimeout(6700);check('failure_remains_after_toast_timeout',await page.locator('.session-error').innerText().then(x=>x.includes('HTTP 503')));
await page.evaluate(()=>{fake.state.session={id:10,phase:'processing'};fake.state.error='';fake.state.result={id:0,created:Date.now()/1000,scene:'chat',layout:'paragraph',revision:0,original:'已经识别出的内容',text:'已经识别出的内容',risks:['unrefined'],delivery:'preview',feedback:false};});await page.waitForTimeout(1100);
await page.evaluate(()=>{fake.state.session.phase='failed';fake.state.error='文字处理超时';});await page.waitForFunction(()=>document.querySelector('.session-error')?.textContent.includes('文字处理超时'));
check('refinement_failure_preserves_raw_and_displays_error',await page.locator('.output').innerText().then(x=>x.includes('已经识别出的内容')));
await page.screenshot({path:path.join(out,'result-recovery.png')});
// Expected input problems stay on the current page, while the dedicated overlay explains them.
await page.locator('.nav [data-page="settings"]').click();
await page.evaluate(()=>{fake.state.session={id:11,phase:'cancelled'};fake.state.result=null;fake.state.error='';fake.state.notice='no_audio';});
await page.waitForTimeout(1200);
check('empty_input_does_not_navigate_main_page',await page.locator('.nav [data-page="settings"]').getAttribute('class').then(x=>x.includes('active')));
await page.setViewportSize({width:380,height:148});await page.goto('file://'+path.join(root,'ui/index.html')+'?bar=1');
await page.locator('.bar-stage').waitFor();
for(const code of ['recording_interrupted','no_audio','no_microphone','no_speech','microphone_unavailable','too_short','selection_unreadable','delivery_target_unavailable','selection_out_of_scope','selection_scope_unavailable']){
 await page.evaluate(code=>{fake.state.session={id:1,phase:'cancelled'};fake.state.notice=code;fake.listeners['session-updated']();},code);
 await page.waitForFunction(()=>!document.querySelector('#bar-notice').classList.contains('hidden'));
 await page.waitForTimeout(120);
 check('overlay_has_text_'+code,await page.locator('#bar-notice').innerText().then(x=>x.length>0));
 check('overlay_fits_'+code,await page.locator('#bar-notice').boundingBox().then(b=>b.x>=0&&b.y>=0&&b.x+b.width<=380&&b.y+b.height<=148));
 check('microphone_is_bubble_only_'+code,await page.locator('.voicebar').isVisible()===!['no_microphone','microphone_unavailable','selection_unreadable','delivery_target_unavailable','selection_out_of_scope','selection_scope_unavailable'].includes(code));
 if(code==='no_audio'||code==='no_microphone'||code==='selection_out_of_scope'||code==='selection_scope_unavailable')await page.screenshot({path:path.join(out,'overlay-'+code+'.png'),omitBackground:true});
}
for(const language of ['zh','en'])for(const theme of ['light','dark'])for(const code of ['selection_out_of_scope','selection_scope_unavailable']){
 await page.evaluate(({language,theme,code})=>{fake.state.uiLanguage=language;fake.state.theme=theme;fake.state.notice=code;fake.listeners['session-updated']();},{language,theme,code});
 await page.waitForFunction(language=>document.querySelector('#bar-message').textContent.includes(language==='zh'?'请':(fake.state.notice==='selection_out_of_scope'?'Ask':'retry')),language);
 check(`scope_notice_fits_${language}_${theme}_${code}`,await page.locator('#bar-notice').boundingBox().then(b=>b.x>=0&&b.y>=0&&b.x+b.width<=380&&b.y+b.height<=148));
 await page.screenshot({path:path.join(out,`scope-${language}-${theme}-${code}.png`),omitBackground:true});
}
await page.evaluate(()=>{fake.state.uiLanguage='zh';fake.state.theme='light';fake.listeners['session-updated']();});
await page.evaluate(()=>{fake.state.notice=null;fake.state.selection=true;fake.state.session.phase='listening';fake.listeners['session-updated']();});
await page.waitForFunction(()=>document.querySelector('#bar-notice').textContent.includes('修改要求'));
check('selected_text_immediately_prompts_voice_instruction',await page.locator('#bar-label').innerText()==='聆听中');
await page.screenshot({path:path.join(out,'overlay-selection.png'),omitBackground:true});
await page.evaluate(()=>{fake.state.selection=false;fake.state.translating=true;fake.state.translationLanguage='English';fake.listeners['session-updated']();});
await page.waitForFunction(()=>document.querySelector('#bar-label').textContent.includes('翻译中'));
check('translation_overlay_shows_mode_and_destination',await page.locator('#bar-label').innerText()==='翻译中 · English');
await page.screenshot({path:path.join(out,'overlay-translation-beta4.png'),omitBackground:true});
await page.evaluate(()=>{fake.state.translating=false;fake.listeners['session-updated']();});
await page.waitForFunction(()=>document.querySelector('#bar-label').textContent==='聆听中');
check('translation_off_returns_to_listening',await page.locator('#bar-label').innerText()==='聆听中');

await page.evaluate(()=>{fake.state.notice='no_microphone';fake.state.uiLanguage='en';fake.state.theme='dark';fake.state.session.phase='cancelled';fake.listeners['session-updated']();});
await page.waitForFunction(()=>document.querySelector('#bar-notice').textContent.includes('No microphone'));
check('bubble_localizes_to_english',await page.locator('#bar-notice').innerText().then(x=>x.includes('Connect')));
// Expanded answers stay readable inside the overlay, with a dedicated drag handle.
await page.setViewportSize({width:520,height:360});
await page.evaluate(()=>{fake.state.notice=null;fake.state.answer=('这是一段很长的追问解释，包含细节与限制。\n').repeat(80)+'<img src=x onerror=alert(1)>';fake.state.session.phase='completed';fake.state.uiLanguage='zh';fake.listeners['session-updated']();});
await page.waitForFunction(()=>document.querySelector('#bar-notice').classList.contains('answer'));
check('answer_escapes_model_text',await page.locator('#bar-message img').count()===0);
check('answer_voice_wave_hidden',!(await page.locator('.voicebar').isVisible()));
for(const viewport of [{width:520,height:360},{width:320,height:200}]) {
 await page.setViewportSize(viewport);
 check('expanded_answer_fits_'+viewport.width,await page.locator('#bar-notice').boundingBox().then(b=>b.x>=0&&b.y>=0&&b.x+b.width<=viewport.width&&b.y+b.height<=viewport.height));
 check('answer_can_scroll_'+viewport.width,await page.locator('#bar-message').evaluate(e=>e.scrollHeight>e.clientHeight&&e.clientHeight>40));
}
await page.setViewportSize({width:520,height:360});
await page.locator('#bar-message').hover();await page.mouse.wheel(0,450);await page.waitForFunction(()=>document.querySelector('#bar-message').scrollTop>0,{},{timeout:2000});
check('answer_real_wheel_scrolls',await page.locator('#bar-message').evaluate(e=>e.scrollTop>0));await page.evaluate(()=>fake.listeners['session-updated']());await page.waitForTimeout(450);check('answer_scroll_survives_snapshot_refresh',await page.locator('#bar-message').evaluate(e=>e.scrollTop>0));
await page.locator('.answer-handle').dispatchEvent('mousedown',{button:0});
check('answer_drag_invokes_native_window_move',await page.evaluate(()=>fake.calls.some(c=>c.command==='drag_voicebar')));
await page.screenshot({path:path.join(out,'overlay-long-answer.png'),omitBackground:true});
await page.locator('#bar-dismiss').click();await page.waitForFunction(()=>document.querySelector('#bar-notice').classList.contains('hidden'));
check('answer_close_dismisses_without_recording',await page.evaluate(()=>fake.calls.some(c=>c.command==='dismiss_answer')&&fake.state.session.phase==='completed'));
await page.setViewportSize({width:380,height:148});
await page.evaluate(()=>{fake.state.answer='';fake.state.notice=null;fake.state.selection=false;fake.state.session.phase='listening';fake.listeners['session-updated']();});
await page.waitForFunction(()=>!document.querySelector('.voicebar').classList.contains('hidden'));
check('next_recording_restores_compact_wave',await page.locator('.voicebar').isVisible()&&!(await page.locator('#bar-notice').isVisible()));
await page.locator('.voicebar').dispatchEvent('mousedown',{button:0});
check('compact_bar_is_draggable',await page.evaluate(()=>fake.calls.filter(c=>c.command==='drag_voicebar').length>=2));
await page.setViewportSize({width:440,height:220});
await page.evaluate(()=>{fake.state.previewStatus='live';fake.state.previewText='项目 Vao 已完成离线检查，真机还没测试。<img src=x onerror=alert(1)>';fake.state.session.phase='listening';fake.state.uiLanguage='zh';fake.listeners['session-updated']();});
await page.waitForFunction(()=>!document.querySelector('#bar-transcript').classList.contains('hidden'));
check('temporary_transcript_is_escaped',await page.locator('#transcript-text img').count()===0&&await page.locator('#transcript-text').innerText().then(t=>t.includes('<img')));
check('preview_panel_fits_native_size',await page.locator('#bar-transcript').boundingBox().then(b=>b.y>=0&&b.x>=0&&b.y+b.height<=220&&b.x+b.width<=440));
check('preview_does_not_open_main',await page.evaluate(()=>!fake.calls.some(c=>c.command==='show_main')));
await page.screenshot({path:path.join(out,'temporary-transcript-zh.png'),omitBackground:true});
await page.evaluate(()=>{fake.state.previewStatus='unavailable';fake.listeners['session-updated']();});await page.waitForFunction(()=>document.querySelector('#transcript-status').textContent.includes('暂不可用'));
check('preview_fallback_is_explained',await page.locator('#transcript-status').innerText().then(t=>t.includes('结束后继续识别')));
await page.evaluate(()=>{fake.state.session.phase='processing';fake.state.previewStatus='finalizing';fake.listeners['session-updated']();});await page.waitForFunction(()=>document.querySelector('#transcript-status').textContent.includes('完整录音'));
check('finalizing_preview_is_temporary',await page.locator('#transcript-caption').innerText().then(t=>t.includes('临时')));
await page.evaluate(()=>{fake.state.session.phase='cancelled';fake.listeners['session-updated']();});await page.waitForFunction(()=>document.querySelector('#bar-transcript').classList.contains('hidden'));
check('cancel_hides_temporary_text',!(await page.locator('#bar-transcript').isVisible()));
check('no_script_errors',errors.length===0);fs.writeFileSync(path.join(out,'ui-checks.json'),JSON.stringify({kind:'browser IPC doubles; not native end-to-end',sources:Object.fromEntries(['ui/index.html','ui/app.js','ui/style.css'].map(f=>[f,require('node:crypto').createHash('sha256').update(fs.readFileSync(path.join(root,f))).digest('hex')])),checks,errors},null,2));await browser.close();console.log(JSON.stringify({passed:checks.length,output:out}));
})().catch(e=>{fs.writeFileSync(path.join(out,'ui-checks-failure.json'),JSON.stringify({checks,error:String(e)},null,2));console.error(e);process.exit(1);});
