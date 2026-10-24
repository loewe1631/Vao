// Browser/IPC tests only; native focus, ASR and real insertion are separate evidence.
const {chromium}=require(process.env.VAO_PLAYWRIGHT_MODULE||'playwright');
const fs=require('node:fs'),path=require('node:path');
const out=path.resolve(__dirname,'../artifacts/checks');fs.mkdirSync(out,{recursive:true});
const checks=[];function check(name,passed){checks.push({name,passed:!!passed});if(!passed)throw Error(name);}
async function setup(page,bar=false){
 await page.addInitScript(()=>{
  const record={id:1,created:1900000000,scene:'chat',layout:'paragraph',revision:0,original:'原始Tablets与华池追问',text:'我们使用Tablets和华池追问。',risks:[],delivery:'preview',feedback:false,edit_revision:0};
  const db={settings:{asr:{base:'',model:'',protocol:'openai_audio'},llm:{base:'',model:'',protocol:'openai_chat'},backup:{base:'',model:'',protocol:'openai_chat'},language:'auto',translation_language:'English',auto_input:true,complete_book_titles:true,dictionary:['Existing hint'],ui_language:'zh',theme:'light',trial_enabled:true,onboarding_complete:true,shortcuts:{record:'Control+Alt+Space',translate:'Control+Alt+KeyT',cancel:'Escape'}},profile:{revision:0,learned_revision:0,learning:true,personalization:true,rules:[],votes:[],undo:[]},history:[record],corrections:{revision:0,enabled:true,groups:[]},platform:'windows',build:'20261024.3018',version:'0.2.6',startup:{available:true,registered:false,system_disabled:false},permissions:{microphone:'granted',accessibility:'granted',bundle:'app.vaovao.windows'},trial:{ready:true,enabled:true,provisioned:true,consented:true}};
  const state={session:{id:1,phase:'idle'},result:record,hotkeys:true};window.fake={db,state,calls:[],listeners:{}};
  window.__TAURI__={event:{listen:async(n,f)=>{fake.listeners[n]=f;}},core:{invoke:async(command,args={})=>{
   fake.calls.push({command,args:structuredClone(args)});
   switch(command){
    case'bootstrap':return structuredClone(db);case'snapshot':return structuredClone(state);case'permissions':return db.permissions;
    case'save_settings':db.settings=args.settings;return;case'update_status':return {phase:'idle'};
    case'save_correction_book':{if(args.revision!==db.corrections.revision)throw 'Library changed';const b=args.book;const keys=new Map();for(const g of b.groups)for(const a of g.aliases){const k=a.wrong.toLowerCase();if(keys.has(k)&&keys.get(k)!==g.correct)throw 'Alias belongs to another group';keys.set(k,g.correct);}fake.undo=db.corrections;db.corrections={...b,revision:db.corrections.revision+1};return structuredClone(db.corrections);}
    case'undo_correction_book':db.corrections={...fake.undo,revision:db.corrections.revision+1};return structuredClone(db.corrections);
    case'save_result_correction':{if(fake.saveFail)throw 'Disk write failed';const r=db.history.find(r=>r.id===args.id);if(r.edit_revision!==args.revision)throw 'Result changed';r.corrected_text=args.text;r.effective_text=args.text;r.edit_revision++;state.result=r;for(const p of args.pairs){let g=db.corrections.groups.find(g=>g.correct===p.correct);if(!g){g={correct:p.correct,enabled:true,aliases:[]};db.corrections.groups.push(g);}if(!g.aliases.some(a=>a.wrong===p.wrong))g.aliases.push({wrong:p.wrong,enabled:true});}if(args.pairs.length)db.corrections.revision++;return structuredClone(r);}
    case'preview_correction_import':return fake.preview||{token:7,added_groups:1,added_aliases:1,duplicates:0,skipped:0,conflicts:[]};
    case'commit_correction_import':if(fake.importFail)throw 'Library changed; preview again';db.corrections.revision++;return {revision:db.corrections.revision};
    case'export_corrections':return {groups:db.corrections.groups.length};
    case'resolve_correction_prompt':if(fake.resolveDeferred)await new Promise(r=>fake.resolve=r);if(fake.resolveFail)throw 'Original target changed';state.session.phase='ready';state.correction=null;return;
    case'cancel':state.session.phase='cancelled';state.correction=null;return;
    default:return;
   }
  }}};
 });
 await page.goto('file://'+path.resolve(__dirname,'../ui/index.html')+(bar?'?bar':''));
 await page.locator(bar?'.voicebar':'h1').waitFor();
}
(async()=>{const browser=await chromium.launch({headless:true});const page=await browser.newPage({viewport:{width:1060,height:820}}),errors=[];page.on('pageerror',e=>errors.push(String(e)));
 try{
 await setup(page);
 await page.locator('[data-correct-result="1"]').click();await page.locator('#corrected-result').fill('我们使用typeless和划词追问。');
 check('editing_does_not_automatically_remember',await page.locator('#correction-pairs input[type=checkbox]:checked').count()===0);
 await page.locator('[data-modal-close]').click();check('cancel_editor_does_not_save',await page.evaluate(()=>!fake.calls.some(c=>c.command==='save_result_correction')));
 await page.locator('[data-correct-result="1"]').click();await page.locator('#corrected-result').fill('我们使用typeless和华池追问。');
 check('single_english_change_suggests_whole_word',await page.locator('#correction-pairs [data-wrong]').inputValue()==='Tablets'&&await page.locator('#correction-pairs [data-correct]').inputValue()==='typeless');
 await page.locator('#result-correction-save').click();await page.locator('#correction-dialog').waitFor({state:'detached'});
 check('save_once_creates_no_memory',await page.evaluate(()=>fake.db.corrections.groups.length===0));
 check('corrected_result_is_displayed',await page.locator('.output').innerText()==='我们使用typeless和华池追问。');
 check('original_model_and_asr_are_preserved',await page.evaluate(()=>fake.db.history[0].text==='我们使用Tablets和华池追问。'&&fake.db.history[0].original==='原始Tablets与华池追问'));
 await page.locator('[data-page="history"]').click();await page.locator('[data-correct-result="1"]').click();await page.locator('#corrected-result').fill('我们使用typeless和划词追问。');
 await page.locator('#correction-pairs [data-wrong]').fill('华池追问');await page.locator('#correction-pairs [data-correct]').fill('划词追问');await page.locator('#correction-pairs input[type=checkbox]').check();
 await page.evaluate(()=>fake.saveFail=true);await page.locator('#result-correction-save').click();await page.locator('.correction-error').filter({hasText:'Disk write'}).waitFor();
 check('failed_save_keeps_editor_and_memory_unchanged',await page.locator('#corrected-result').inputValue()==='我们使用typeless和划词追问。'&&await page.evaluate(()=>fake.db.corrections.groups.length===0));
 await page.evaluate(()=>fake.saveFail=false);await page.locator('#result-correction-save').click();await page.locator('#correction-dialog').waitFor({state:'detached'});
 check('explicitly_checked_pair_is_remembered',await page.evaluate(()=>fake.calls.filter(c=>c.command==='save_result_correction').at(-1).args.pairs.length===1&&fake.db.corrections.groups[0].correct==='划词追问'));
 await page.locator('[data-page="dictionary"]').click();check('existing_word_dictionary_is_preserved',await page.locator('#dictionary').inputValue()==='Existing hint');
 await page.locator('[data-correction-tab="corrections"]').click();await page.locator('#correction-add').click();await page.locator('#group-correct').fill('划词追问');await page.locator('#group-wrong').fill('华慈追问\n话刺追问');await page.locator('#group-save').click();await page.locator('#correction-dialog').waitFor({state:'detached'});
 check('same_target_automatically_groups_aliases',await page.evaluate(()=>fake.db.corrections.groups.length===1&&fake.db.corrections.groups[0].aliases.length===3));
 await page.locator('[data-alias-toggle="0:1"]').uncheck();await page.waitForFunction(()=>!fake.db.corrections.groups[0].aliases[1].enabled);check('alias_can_be_disabled',true);
 await page.locator('#correction-enabled').click();await page.waitForFunction(()=>!fake.db.corrections.enabled);check('global_suggestions_can_be_disabled',true);
 await page.locator('#correction-undo').click();await page.waitForFunction(()=>fake.db.corrections.enabled);check('undo_restores_previous_setting',true);
 await page.locator('[data-group-select="0"]').check();await page.locator('#correction-export-selected').click();check('selected_export_sends_only_chosen_targets',await page.evaluate(()=>fake.calls.findLast(c=>c.command==='export_corrections').args.selected.join()==='划词追问'));
 await page.locator('[data-group-edit="0"]').click();await page.locator('#group-correct').fill('划词追问 修订');await page.locator('#group-save').click();await page.locator('#correction-dialog').waitFor({state:'detached'});check('renaming_prunes_stale_export_selection',await page.locator('#correction-export-selected').isDisabled()&&!await page.locator('[data-group-select="0"]').isChecked());
 await page.locator('#correction-export').click();check('all_export_is_explicit',await page.evaluate(()=>fake.calls.findLast(c=>c.command==='export_corrections').args.selected===null));
 await page.locator('#correction-search').fill('话刺');check('alias_search_finds_group',await page.locator('.correction-group').count()===1);await page.locator('#correction-search').fill('nothing');check('search_can_return_empty',await page.locator('.correction-group').count()===0);await page.locator('#correction-search').fill('');
 await page.locator('#correction-import').click();check('preview_does_not_commit',await page.evaluate(()=>!fake.calls.some(c=>c.command==='commit_correction_import')));await page.locator('[data-modal-close]').click();await page.waitForFunction(()=>fake.calls.some(c=>c.command==='cancel_correction_import'));check('cancel_import_discards_preview',true);
 await page.evaluate(()=>fake.preview={token:8,added_groups:0,added_aliases:0,duplicates:0,skipped:1,conflicts:[{key:'tablets',wrong:'Tablets',existing:'typeless',incoming:'tablets app'}]});
 await page.locator('#correction-import').click();await page.locator('#correction-import-confirm').click();await page.locator('.correction-error').filter({hasText:'冲突'}).waitFor();check('import_conflict_requires_explicit_choice',await page.evaluate(()=>!fake.calls.some(c=>c.command==='commit_correction_import')));
 await page.locator('[data-import-choice]').selectOption('typeless');await page.locator('#correction-import-confirm').click();await page.locator('#correction-dialog').waitFor({state:'detached'});check('import_passes_reviewed_choice',await page.evaluate(()=>fake.calls.findLast(c=>c.command==='commit_correction_import').args.choices.tablets==='typeless'));
 await page.locator('[data-group-delete="0"]').click();check('delete_requires_visible_confirmation',await page.locator('#correction-delete-confirm').isVisible());await page.locator('[data-modal-close]').click();check('cancel_delete_preserves_library',await page.evaluate(()=>fake.db.corrections.groups.length===1));
 await page.locator('#lang').click();check('english_library_labels',await page.locator('#correction-add').innerText()==='Add correction');await page.locator('#lang').click();
 await page.evaluate(()=>{fake.state.session={id:2,phase:'awaiting_correction'};fake.state.correction={session_id:2,book_revision:3,auto_input:true,items:[{id:0,found:'Tablets',correct:'typeless',before:'我们使用',after:'。',overlaps:[]}]};fake.listeners['session-updated']?.();});
 await page.locator('[data-correction-apply]').waitFor();check('pending_disables_new_recording',await page.locator('#record').isDisabled());check('single_candidate_is_ready_for_one_click',await page.locator('[data-candidate="0"]').isChecked());
 await page.waitForTimeout(1300);check('pending_has_no_automatic_send_or_countdown',await page.evaluate(()=>!fake.calls.some(c=>c.command==='resolve_correction_prompt'))&&!(await page.locator('.correction-review').innerText()).includes('秒'));
 await page.evaluate(()=>fake.resolveDeferred=true);await page.locator('[data-correction-apply]').dblclick();await page.waitForFunction(()=>fake.resolve);check('double_click_sends_one_confirmation',await page.evaluate(()=>fake.calls.filter(c=>c.command==='resolve_correction_prompt').length===1));await page.evaluate(()=>fake.resolve());await page.locator('[data-correction-apply]').waitFor({state:'detached'});
 const bar=await browser.newPage({viewport:{width:380,height:360}});bar.on('pageerror',e=>errors.push(String(e)));await setup(bar,true);
 await bar.evaluate(()=>{fake.state.session={id:5,phase:'awaiting_correction'};fake.state.correction={session_id:5,auto_input:true,items:[{id:0,found:'cloud file',correct:'cloudflare',before:'<img src=x onerror=alert(1)>',after:'。',overlaps:[1]},{id:1,found:'file',correct:'document',before:'cloud ',after:'。',overlaps:[0]},{id:2,found:'小明',correct:'晓明',before:'你好',after:'',overlaps:[]}]};fake.listeners['session-updated']?.();});
 await bar.locator('[data-candidate="0"]').waitFor();check('bar_hides_wave_while_waiting',await bar.locator('.voicebar').isHidden());check('candidate_text_is_escaped',await bar.locator('.correction-review img').count()===0&&await bar.locator('.correction-review').innerText().then(t=>t.includes('<img')));
 check('multiple_candidates_require_selection',await bar.locator('[data-correction-apply]').isDisabled());await bar.locator('[data-candidate="0"]').check();await bar.locator('[data-candidate="1"]').check();check('overlapping_candidates_are_mutually_exclusive',!await bar.locator('[data-candidate="0"]').isChecked()&&await bar.locator('[data-candidate="1"]').isChecked());await bar.locator('[data-candidate="2"]').check();
 await bar.waitForTimeout(350);check('poll_preserves_selected_candidates',await bar.locator('[data-candidate="1"]').isChecked()&&await bar.locator('[data-candidate="2"]').isChecked());
 check('windows_prompt_requests_native_height',await bar.evaluate(()=>fake.calls.some(c=>c.command==='fit_voicebar'&&c.args.height>74&&c.args.height<=360)));
 check('all_actions_fit_native_overlay',await bar.locator('[data-correction-cancel]').evaluate(e=>e.getBoundingClientRect().bottom<=innerHeight));
 await bar.screenshot({path:path.join(out,'corrections-overlay.png')});
 await bar.locator('[data-correction-keep]').click();await bar.locator('[data-correction-keep]').waitFor({state:'detached'});check('keep_sends_empty_selection',await bar.evaluate(()=>fake.calls.find(c=>c.command==='resolve_correction_prompt').args.selected.length===0));
 await bar.evaluate(()=>{fake.state.session={id:6,phase:'awaiting_correction'};fake.state.correction={session_id:6,auto_input:true,items:[{id:0,found:'Tablets',correct:'typeless',before:'',after:'',overlaps:[]}]};fake.listeners['session-updated']?.();});await bar.locator('[data-correction-cancel]').click();check('cancel_never_confirms_input',await bar.evaluate(()=>fake.calls.filter(c=>c.command==='resolve_correction_prompt').length===1&&fake.calls.some(c=>c.command==='cancel')));
 await bar.evaluate(()=>{fake.state.uiLanguage='en';fake.state.session={id:7,phase:'awaiting_correction'};fake.state.correction={session_id:7,auto_input:true,items:Array.from({length:80},(_,id)=>({id,found:'Example mistaken phrase',correct:'Correct phrase',before:'A longer context before the term ',after:' with a longer context afterwards.',overlaps:[]}))};fake.listeners['session-updated']?.();});
 await bar.locator('[data-candidate="79"]').waitFor();
 check('windows_380dip_many_candidates_scroll',await bar.locator('.correction-review-items').evaluate(e=>e.scrollHeight>e.clientHeight));
 check('windows_380dip_english_actions_fit',await bar.locator('[data-correction-cancel]').evaluate(e=>{const r=e.getBoundingClientRect();return r.bottom<=innerHeight&&r.right<=innerWidth&&r.left>=0;}));
 check('windows_card_width_stays_inside_overlay',await bar.locator('.correction-review').evaluate(e=>{const r=e.getBoundingClientRect();return r.width<=innerWidth&&r.left>=0&&r.right<=innerWidth;}));
 await page.locator('[data-page="dictionary"]').click();await page.screenshot({path:path.join(out,'corrections-library.png')});
 check('no_javascript_errors',errors.length===0);
 }catch(e){checks.push({name:'execution',passed:false,error:String(e)});await page.screenshot({path:path.join(out,'corrections-failure.png')});process.exitCode=1;}
 finally{fs.writeFileSync(path.join(out,'corrections-ui.json'),JSON.stringify({evidence:'browser with IPC doubles; not native/audio acceptance',checks,errors},null,2));console.log(JSON.stringify({passed:checks.every(c=>c.passed),checks:checks.length,failed:checks.filter(c=>!c.passed),errors}));await browser.close();}
})();
