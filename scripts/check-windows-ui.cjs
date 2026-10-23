// Browser IPC doubles for Windows UI; not a native microphone/focus acceptance.
const {chromium}=require(process.env.VAO_PLAYWRIGHT_MODULE||'playwright');
const fs=require('node:fs'),path=require('node:path');
const root=path.resolve(__dirname,'..'),out=path.join(root,'artifacts/checks');
fs.mkdirSync(out,{recursive:true});
const checks=[],errors=[];
function check(name,passed){checks.push({name,passed:!!passed});if(!passed)throw Error(name);}
(async()=>{
 const browser=await chromium.launch({headless:true});
 const page=await browser.newPage({viewport:{width:1080,height:780}});
 page.on('pageerror',error=>errors.push(String(error)));
 await page.addInitScript(()=>{
  const connection={base:'https://model.example/v1',model:'synthetic',protocol:'openai_chat'};
  const db={platform:'windows',version:'0.2.2',build:'20261022.3014',
   settings:{asr:{...connection,protocol:'openai_audio'},llm:connection,backup:{base:'',model:'',protocol:'openai_chat'},language:'auto',translation_language:'English',auto_input:true,windows_compatible_input:true,windows_return_to_origin:false,complete_book_titles:true,dictionary:[],ui_language:'zh',theme:'system',trial_enabled:false,trial_consent_version:'',onboarding_complete:false,shortcuts:{record:'Control+Alt+Space',translate:'Control+Alt+KeyT',cancel:'Escape'}},
   startup:{available:true,registered:false,system_disabled:false},
   profile:{revision:0,learning:true,personalization:true,rules:[],votes:[],undo:[]},history:[],
   permissions:{microphone:'not_determined',accessibility:'not_supported',input_monitoring:'not_required'},
   trial:{provisioned:false,consented:false,ready:true,asr_source:'custom',llm_source:'custom'}};
  const state={session:{id:0,phase:'idle'},hotkeys:true,result:null,level:0,error:'',translating:false};
  window.fake={db,state,calls:[],listeners:{}};
  window.__TAURI__={event:{listen:async(name,fn)=>{fake.listeners[name]=fn;return ()=>{};}},core:{invoke:async(command,args={})=>{
   fake.calls.push({command,args});
   switch(command){
    case 'bootstrap':return structuredClone(db);
    case 'snapshot':return structuredClone(state);
    case 'permissions':return structuredClone(db.permissions);
    case 'key_status':return {asr:true,llm:true,backup:false};
    case 'request_permission':db.permissions.microphone='granted';return structuredClone(db.permissions);
    case 'save_shortcuts':if(fake.conflict)throw 'Shortcut unavailable';if(Object.values(args.shortcuts).some(x=>x.includes('Fn')))throw 'Fn is unavailable on Windows';db.settings.shortcuts=args.shortcuts;return {saved:true,active:true};
    case 'save_settings':db.settings=args.settings;return;
    case 'login_startup_status':return structuredClone(db.startup);
    case 'set_login_startup':if(fake.startupError)throw 'Startup change failed';db.startup={available:true,registered:args.enabled,system_disabled:args.enabled&&db.startup.system_disabled};return structuredClone(db.startup);
    case 'open_startup_settings':return;
    case 'toggle_recording':state.session={id:1,phase:state.session.phase==='listening'?'processing':'listening'};return;
    case 'cancel':state.session.phase='cancelled';return;
    case 'dismiss_answer':state.answer='';return;
    case 'update_status':return structuredClone(fake.update||{phase:'idle'});
    case 'check_update':if(fake.updateFail)throw '尚未发布 Windows 更新清单';fake.update={phase:'available',version:'0.2.3',notes:'<script>synthetic unsafe notes</script>',total:512};return structuredClone(fake.update);
    case 'download_update':fake.update={...fake.update,phase:'ready',downloaded:512};return structuredClone(fake.update);
    case 'install_update':if(fake.installBlocked)throw '请先结束语音输入并处理待回填结果';fake.update={...fake.update,phase:'installing'};return;
    default:return;
   }
  }}};
 });
 await page.goto('file://'+path.join(root,'ui/index.html'));
 await page.locator('.sidebar-foot').waitFor();
 check('release_has_no_development_banner',await page.locator('.windows-preview').count()===0);
 check('release_shows_clean_version',await page.locator('.sidebar-foot').innerText()==='0.2.2');
 check('release_brand_has_no_beta_label',!(await page.locator('.sidebar .brand').innerText()).includes('BETA'));
 check('no_enrollment_or_copy_on_launch',await page.evaluate(()=>!fake.calls.some(c=>['enroll_trial','copy_result'].includes(c.command))));
 check('onboarding_waits_for_actual_probe',await page.locator('#finish-setup').isDisabled());
 check('onboarding_offers_insertion',await page.locator('#setup-auto-input').count()===1);
 await page.locator('.nav [data-page="settings"]').click();
 check('legacy_auto_stop_default_off',await page.locator('#auto-stop').getAttribute('aria-checked')==='false');
 check('auto_stop_delay_hidden_when_off',await page.locator('#silence-seconds').count()===0);
 await page.locator('#auto-stop').click();
 check('auto_stop_saved_without_changing_input_strategy',await page.evaluate(()=>fake.db.settings.auto_stop===true&&!fake.db.settings.windows_return_to_origin&&fake.db.settings.windows_compatible_input));
 check('legacy_auto_stop_delay_is_five',await page.locator('#silence-seconds').inputValue()==='5');
 for(const value of ['2','30','17']){
  await page.locator('#silence-seconds').fill(value);await page.locator('#save-silence-seconds').click();
  check('auto_stop_delay_saved_'+value,await page.evaluate(v=>fake.db.settings.silence_seconds===Number(v),value));
 }
 for(const value of ['1','31','2.5','']){
  await page.locator('#silence-seconds').fill(value);await page.locator('#save-silence-seconds').click();
  check('auto_stop_invalid_delay_rejected_'+value,await page.evaluate(()=>fake.db.settings.silence_seconds===17));
 }
 await page.locator('#silence-seconds').fill('5');await page.locator('#save-silence-seconds').click();
 await page.locator('.nav [data-page="home"]').click();await page.locator('.nav [data-page="settings"]').click();
 check('auto_stop_saved_across_navigation',await page.locator('#silence-seconds').inputValue()==='5');
 await page.locator('#auto-stop').click();
 check('auto_stop_can_be_disabled',await page.locator('#silence-seconds').count()===0&&await page.evaluate(()=>fake.db.settings.auto_stop===false));
 check('login_startup_default_off',await page.locator('#login-startup').getAttribute('aria-checked')==='false');
 check('launch_does_not_register_startup',await page.evaluate(()=>!fake.calls.some(c=>c.command==='set_login_startup')));
 await page.locator('#login-startup').click();
 check('login_startup_opt_in_registers',await page.evaluate(()=>fake.db.startup.registered&&fake.calls.filter(c=>c.command==='set_login_startup').length===1));
 await page.evaluate(()=>fake.startupError=true);
 await page.locator('#login-startup').click();
 check('startup_failure_keeps_checked_state',await page.locator('#login-startup').getAttribute('aria-checked')==='true');
 await page.evaluate(()=>fake.startupError=false);
 await page.locator('#login-startup').click();
 check('login_startup_can_be_disabled',await page.evaluate(()=>!fake.db.startup.registered));
 await page.evaluate(()=>{fake.db.startup={available:true,registered:true,system_disabled:true};window.dispatchEvent(new Event('focus'));});
 await page.locator('#startup-system-settings').waitFor();
 check('system_disable_has_explicit_settings_link',await page.locator('#startup-card').innerText().then(s=>s.includes('Windows 已禁用')));
 await page.locator('#startup-system-settings').click();
 check('startup_system_settings_only_on_click',await page.evaluate(()=>fake.calls.filter(c=>c.command==='open_startup_settings').length===1));
 await page.locator('#login-startup').click();
 check('system_disabled_entry_can_be_removed',await page.evaluate(()=>!fake.db.startup.registered));
 check('startup_does_not_reset_model_or_shortcuts',await page.evaluate(()=>fake.db.settings.asr.model==='synthetic'&&fake.db.settings.shortcuts.record==='Control+Alt+Space'));
 check('current_caret_default',await page.locator('#return-to-origin').getAttribute('aria-checked')==='false');
 await page.locator('#return-to-origin').click();
 await page.waitForFunction(()=>fake.db.settings.windows_return_to_origin===true);
 check('return_to_origin_persists',await page.locator('#return-to-origin').getAttribute('aria-checked')==='true');
 await page.locator('#return-to-origin').click();
 await page.waitForFunction(()=>fake.db.settings.windows_return_to_origin===false);
 check('compatible_paste_default_on',await page.locator('#compatible-input').getAttribute('aria-checked')==='true');
 check('input_enhancements_removed',await page.locator('#live-preview,#input-context,#expanded-context,#bar-transcript').count()===0);
 check('smart_completion_default_on',await page.locator('#complete-book-titles').getAttribute('aria-checked')==='true');
 await page.locator('#complete-book-titles').click();
 await page.waitForFunction(()=>fake.db.settings.complete_book_titles===false);
 check('smart_completion_opt_out_keeps_windows_mode',await page.evaluate(()=>fake.db.settings.windows_return_to_origin===false&&fake.db.settings.windows_compatible_input===true));
 await page.locator('#complete-book-titles').click();
 await page.waitForFunction(()=>fake.db.settings.complete_book_titles===true);
 check('automatic_input_label_has_no_origin_promise',await page.locator('#auto-input').locator('..').locator('h3').innerText()==='自动输入');
 check('automatic_input_explains_completion_caret',await page.locator('#auto-input').locator('..').innerText().then(s=>s.includes('结果完成时')&&s.includes('当前光标位置')));
 await page.locator('#compatible-input').click();
 await page.waitForFunction(()=>fake.db.settings.windows_compatible_input===false);
 check('compatible_paste_can_be_disabled',await page.locator('#compatible-input').getAttribute('aria-checked')==='false');
 await page.locator('#compatible-input').click();
 await page.waitForFunction(()=>fake.db.settings.windows_compatible_input===true);
 check('compatible_paste_can_be_restored',await page.locator('#compatible-input').getAttribute('aria-checked')==='true');
 check('compatible_paste_does_not_reset_connections',await page.evaluate(()=>fake.db.settings.asr.model==='synthetic'&&fake.db.settings.shortcuts.record==='Control+Alt+Space'));
 check('windows_has_one_permission_row',await page.locator('.permission-row').count()===1);
 check('no_macos_reset_actions',await page.locator('[data-repair-permission]').count()===0);
 check('automatic_insertion_toggle',await page.locator('#auto-input').getAttribute('aria-checked')==='true');
 await page.locator('#auto-input').click();
 check('explicit_disable_preserved',await page.evaluate(()=>fake.db.settings.auto_input===false));
 await page.locator('#auto-input').click();
 check('windows_settings_have_no_macos_directions',!(await page.locator('#permissions-card').innerText()).includes('macOS'));
 check('plain_windows_modifier_labels',await page.locator('[data-shortcut-label="record"]').innerText()==='Ctrl+Alt+Space');
 await page.locator('[data-request-permission="microphone"]').click();
 await page.waitForFunction(()=>document.querySelector('[data-permission-status="microphone"]').textContent==='访问检查已通过');
 check('permission_probe_is_explicit',await page.evaluate(()=>fake.calls.filter(c=>c.command==='request_permission').length===1));
 await page.locator('[data-edit-shortcut="record"]').click();
 check('no_windows_fn_presets',await page.locator('[data-shortcut-preset="Fn"]').count()===0);
 await page.locator('#capture-shortcut').click();
 await page.keyboard.press('Control+Shift+K');
 check('windows_webview_captures_normal_chord',await page.locator('#shortcut-input').inputValue()==='Control+Shift+KeyK');
 await page.evaluate(()=>fake.conflict=true);
 await page.locator('#save-shortcut').click();
 await page.locator('.toast').waitFor();
 check('conflict_keeps_previous_binding',await page.evaluate(()=>fake.db.settings.shortcuts.record==='Control+Alt+Space'));
 await page.evaluate(()=>fake.conflict=false);
 await page.locator('#save-shortcut').click();
 await page.waitForFunction(()=>fake.db.settings.shortcuts.record==='Control+Shift+KeyK');
 check('custom_shortcut_can_be_saved',true);
 await page.locator('.nav [data-page="home"]').click();
 check('ready_does_not_require_macos_accessibility',await page.locator('#top-status').innerText()==='可开始录音');
 await page.locator('#lang').click();
 check('english_release_has_no_development_banner',await page.locator('.windows-preview').count()===0);

 await page.locator('.nav [data-page="about"]').click();
 check('windows_updates_are_available_in_about',await page.locator('#check-update').count()===1);
 check('updates_are_not_checked_automatically',await page.evaluate(()=>!fake.calls.some(c=>c.command==='check_update')));
 await page.evaluate(()=>fake.updateFail=true);await page.locator('#check-update').click();
 await page.waitForFunction(()=>document.querySelector('#update-message').textContent.includes('尚未发布'));
 check('missing_feed_is_not_claimed_current',!(await page.locator('#update-message').innerText()).includes('up to date'));
 await page.evaluate(()=>fake.updateFail=false);await page.locator('#check-update').click();
 await page.locator('#download-update').waitFor();
 check('signed_release_notes_render_as_text',await page.locator('#update-message').locator('..').innerText().then(s=>s.includes('<script>synthetic unsafe notes</script>'))&&await page.locator('#update-message').locator('..').locator('script').count()===0);
 await page.locator('#download-update').click();await page.locator('#install-update').waitFor();
 await page.evaluate(()=>{window.confirm=()=>{throw Error('Native confirm must not be used');};});
 await page.locator('#install-update').click();
 check('update_install_requires_in_app_confirmation',await page.locator('#confirm-install-update').count()===1&&await page.evaluate(()=>!fake.calls.some(c=>c.command==='install_update')));
 check('update_cancel_gets_default_focus',await page.locator('#cancel-install-update').evaluate(e=>e===document.activeElement));
 await page.locator('#cancel-install-update').click();
 check('cancel_update_preserves_verified_download',await page.locator('#install-update').count()===1&&await page.evaluate(()=>fake.update.phase==='ready'&&!fake.calls.some(c=>c.command==='install_update')));
 await page.locator('#install-update').click();await page.evaluate(()=>fake.installBlocked=true);await page.locator('#confirm-install-update').click();
 await page.waitForFunction(()=>document.querySelector('#update-message').textContent.includes('待回填'));
 check('update_idle_gate_failure_is_visible',true);
 await page.evaluate(()=>fake.installBlocked=false);await page.locator('#check-update').click();await page.locator('#download-update').click();await page.locator('#install-update').click();
 await page.locator('#confirm-install-update').dblclick({delay:20,noWaitAfter:true}).catch(()=>{});
 check('confirmed_installer_is_dispatched_once',await page.evaluate(()=>fake.calls.filter(c=>c.command==='install_update').length===2));
 check('updater_does_not_modify_input_settings',await page.evaluate(()=>!fake.db.settings.windows_return_to_origin&&fake.db.settings.windows_compatible_input&&fake.db.settings.auto_input));

 for(const language of ['en','zh']){
  if(await page.evaluate(()=>fake.db.settings.ui_language)!==language)await page.locator('#lang').click();
  await page.locator('.nav [data-page="guide"]').click();
  for(const lesson of ['dictate','translate']){
   await page.locator(`[data-lesson="${lesson}"]`).click();
   const count=await page.locator('[data-demo-step]').count();
   check(`guide_${language}_${lesson}_steps`,count===(lesson==='dictate'?5:7));
   await page.locator('#demo-next').click();
   check(`guide_${language}_${lesson}_next`,await page.locator('#demo-progress').innerText()===`2 / ${count}`);
   await page.locator('#demo-prev').click();
   await page.locator(`[data-demo-step="${count-1}"]`).click();
   check(`guide_${language}_${lesson}_last`,await page.locator('#demo-next').isDisabled());
   await page.locator('#demo-replay').click();
   check(`guide_${language}_${lesson}_replay`,await page.locator('#demo-progress').innerText()===`1 / ${count}`);
   await page.locator('#demo-play').click();
   await page.waitForFunction(()=>document.querySelector('#demo-progress').textContent.startsWith('2 /'),{},{timeout:6000});
   await page.locator('#demo-play').click();
   check(`guide_${language}_${lesson}_play_pause`,await page.locator('#demo-progress').innerText()===`2 / ${count}`);
  }
  await page.locator('.nav [data-page="settings"]').click();
  check(`guide_${language}_navigation_returns`,await page.locator('#auto-input').count()===1);
  check(`automatic_input_label_${language}`,await page.locator('#auto-input').locator('..').locator('h3').innerText()===(language==='en'?'Automatic input':'自动输入'));
  check(`auto_stop_label_${language}`,await page.locator('#auto-stop').locator('..').locator('h3').innerText()===(language==='en'?'Stop after a pause':'停说后自动结束'));
  check(`login_startup_label_${language}`,await page.locator('#login-startup').locator('..').locator('h3').innerText()===(language==='en'?'Start when signing in to Windows':'随 Windows 登录启动'));
  for(const route of ['home','guide','habits','history','dictionary','models','settings','about']){
   await page.locator(`.nav [data-page="${route}"]`).click();
   const text=await page.locator('.page').innerText();
   check(`public_copy_${language}_${route}`,!/(Windows 预览|Windows preview|开发预览|development preview|请在笔记本实测|test on your PC|公开测试版|public beta|实验|experimental|BETA|20261022\.3014)/i.test(text));
   if(['settings','about'].includes(route))await page.screenshot({path:path.join(out,`windows-public-${language}-${route}.png`),fullPage:true});
  }
 }
 await page.goto('file://'+path.join(root,'ui/index.html')+'?bar=1');
 await page.waitForFunction(()=>!!fake.listeners['session-updated']);
 for(const language of ['zh','en']){
  await page.evaluate(lang=>{fake.state.session={id:2,phase:'listening'};fake.state.uiLanguage=lang;fake.state.autoStopCountdown=2;fake.listeners['session-updated']();},language);
  await page.waitForFunction(()=>document.querySelector('#bar-label').textContent.includes('2'));
  check('auto_stop_countdown_visible_'+language,await page.locator('#bar-label').innerText().then(x=>x.includes(language==='zh'?'即将结束':'Ending in')));
  await page.evaluate(()=>{fake.state.translating=true;fake.listeners['session-updated']();});
  await page.waitForFunction(()=>/翻译中|Translating/.test(document.querySelector('#bar-label').textContent));
  check('translation_hides_pause_countdown_'+language,!/即将结束|Ending in|2/.test(await page.locator('#bar-label').innerText()));
  await page.evaluate(()=>{fake.state.translating=false;fake.state.autoStopCountdown=null;fake.listeners['session-updated']();});
  await page.waitForFunction(()=>!document.querySelector('#bar-label').textContent.includes('2'));
  check('auto_stop_resuming_voice_clears_countdown_'+language,await page.locator('.voicebar').isVisible());
  await page.evaluate(()=>{fake.state.autoStopUnavailable=true;fake.listeners['session-updated']();});
  await page.waitForFunction(()=>/shortcut|快捷键/.test(document.querySelector('#bar-label').textContent));
  check('auto_stop_failure_retains_manual_recording_'+language,await page.locator('.voicebar').isVisible());
  await page.evaluate(()=>{fake.state.autoStopUnavailable=false;fake.listeners['session-updated']();});
 }
 check('countdown_does_not_open_main_or_submit_from_ui',await page.evaluate(()=>!fake.calls.some(c=>['show_main','toggle_recording'].includes(c.command))));
 await page.evaluate(()=>{fake.state.uiLanguage='zh';});
 await page.evaluate(()=>{fake.state.session={id:3,phase:'completed'};fake.state.answer='A saved result.';fake.state.result={id:3,text:'A saved result.',original:'synthetic',risks:[],delivery:'preview',scene:'chat',revision:0};fake.listeners['session-updated']();});
 await page.waitForFunction(()=>document.querySelector('#bar-message')?.textContent==='A saved result.');
 check('overlay_does_not_claim_implicit_copy',await page.locator('#bar-message').innerText()==='A saved result.');
 check('overlay_copy_is_explicit',await page.evaluate(()=>!fake.calls.some(c=>c.command==='copy_result')));
 await page.locator('#bar-copy').click();
 check('overlay_copies_saved_result',await page.evaluate(()=>fake.calls.some(c=>c.command==='copy_result'&&c.args.id===3)));
 await page.evaluate(()=>{fake.state.session.id=4;fake.state.result.id=4;fake.state.answer='A second result.';fake.listeners['session-updated']();});
 await page.waitForFunction(()=>document.querySelector('#bar-message')?.textContent==='A second result.');
 check('new_result_resets_copy_confirmation',await page.locator('#bar-copy').innerText()==='复制结果');

 await page.waitForFunction(()=>fake.calls.some(c=>c.command==='fit_voicebar'));
 const shortHeight=await page.evaluate(()=>fake.calls.filter(c=>c.command==='fit_voicebar').at(-1).args.height);
 check('short_answer_content_fit',shortHeight>=74&&shortHeight<220);
 await page.evaluate(()=>{fake.state.answer='Long synthetic text. '.repeat(400);fake.state.error='target conflict';fake.listeners['session-updated']();});
 await page.waitForFunction(()=>fake.calls.filter(c=>c.command==='fit_voicebar').at(-1).args.height===360);
 await page.setViewportSize({width:380,height:360});
 check('long_answer_scrolls',await page.locator('#bar-message').evaluate(e=>e.scrollHeight>e.clientHeight));
 check('failed_delivery_shows_copy_hint',await page.locator('#bar-delivery-hint').isVisible());
 await page.evaluate(()=>{fake.state.answer='Short again.';fake.state.error='';fake.listeners['session-updated']();});
 await page.waitForFunction(()=>fake.calls.filter(c=>c.command==='fit_voicebar').at(-1).args.height<220);
 check('answer_shrinks_after_long_result',true);
 await page.locator('.answer-handle').dispatchEvent('mousedown',{button:0});
 check('overlay_drag_does_not_open_main',await page.evaluate(()=>fake.calls.some(c=>c.command==='drag_voicebar')&&!fake.calls.some(c=>c.command==='show_main')));
 await page.evaluate(()=>{fake.state.answer='';fake.state.error='';fake.state.notice=null;fake.state.result.delivery='paste_unconfirmed';fake.listeners['session-updated']();});
 await page.waitForFunction(()=>document.querySelector('#bar-notice').classList.contains('hidden'));
 check('completed_paste_has_no_bubble',await page.locator('#bar-notice').isHidden());
 check('completed_paste_has_no_large_result',await page.locator('.answer-toolbar').isHidden());
 check('unconfirmed_paste_keeps_history_result',await page.evaluate(()=>fake.state.result.id===4&&fake.state.result.delivery==='paste_unconfirmed'));
 await page.screenshot({path:path.join(out,'windows-result-overlay.png')});
 check('no_script_errors',errors.length===0);
 fs.writeFileSync(path.join(out,'windows-ui-checks.json'),JSON.stringify({kind:'Windows browser IPC doubles; not native acceptance',checks,errors},null,2));
 await browser.close();console.log(JSON.stringify({passed:checks.length}));
})().catch(error=>{fs.writeFileSync(path.join(out,'windows-ui-failure.json'),JSON.stringify({checks,error:String(error)},null,2));console.error(error);process.exit(1);});
