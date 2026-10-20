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
  const db={platform:'windows',version:'0.1.0-beta.5-windows.3',build:'20261020.3003',
   settings:{asr:{...connection,protocol:'openai_audio'},llm:connection,backup:{base:'',model:'',protocol:'openai_chat'},language:'auto',translation_language:'English',auto_input:true,dictionary:[],ui_language:'zh',theme:'system',trial_enabled:false,trial_consent_version:'',onboarding_complete:false,shortcuts:{record:'Control+Alt+Space',translate:'Control+Alt+KeyT',cancel:'Escape'}},
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
    case 'toggle_recording':state.session={id:1,phase:state.session.phase==='listening'?'processing':'listening'};return;
    case 'cancel':state.session.phase='cancelled';return;
    case 'dismiss_answer':state.answer='';return;
    default:return;
   }
  }}};
 });
 await page.goto('file://'+path.join(root,'ui/index.html'));
 await page.locator('.windows-preview').waitFor();
 check('preview_identifies_native_field_scope',await page.locator('.windows-preview').innerText().then(s=>s.includes('原生文本框回填')&&s.includes('3003')));
 check('no_enrollment_or_copy_on_launch',await page.evaluate(()=>!fake.calls.some(c=>['enroll_trial','copy_result'].includes(c.command))));
 check('onboarding_waits_for_actual_probe',await page.locator('#finish-setup').isDisabled());
 check('onboarding_offers_insertion',await page.locator('#setup-auto-input').count()===1);
 await page.locator('.nav [data-page="settings"]').click();
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
 check('ready_does_not_require_macos_accessibility',await page.locator('#top-status').innerText().then(s=>s.includes('原生文本框')));
 await page.locator('#lang').click();
 check('english_preview_is_explicit',await page.locator('.windows-preview').innerText().then(s=>s.includes('native text-field insertion')));

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
 }
 await page.goto('file://'+path.join(root,'ui/index.html')+'?bar=1');
 await page.waitForFunction(()=>!!fake.listeners['session-updated']);
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
 await page.screenshot({path:path.join(out,'windows-result-overlay.png')});
 check('no_script_errors',errors.length===0);
 fs.writeFileSync(path.join(out,'windows-ui-checks.json'),JSON.stringify({kind:'Windows browser IPC doubles; not native acceptance',checks,errors},null,2));
 await browser.close();console.log(JSON.stringify({passed:checks.length}));
})().catch(error=>{fs.writeFileSync(path.join(out,'windows-ui-failure.json'),JSON.stringify({checks,error:String(error)},null,2));console.error(error);process.exit(1);});
