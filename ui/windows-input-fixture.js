// Runs only in the native --windows-input-probe fixture. No user input is read.
(async()=>{
 const invoke=window.__TAURI__.core.invoke;
 const mode=await invoke('probe_windows_input',{action:'mode',request:null});
 if(mode.paste_preview){await import('./windows-paste-fixture.js');return;}
 if(mode.unicode_preview){await import('./windows-unicode-fixture.js');return;}
 const checks=[]; let enters=0,submits=0,inputs=0,model='';
 const one=document.querySelector('#one'),two=document.querySelector('#two');
 document.addEventListener('keydown',e=>{if(e.key==='Enter')enters++;});
 document.querySelector('#form').addEventListener('submit',e=>{e.preventDefault();submits++;});
 one.addEventListener('input',()=>{inputs++;model=one.value;});
 const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
 const call=(action,request)=>invoke('probe_windows_input',{action,request:request||null});
 function check(name,passed){checks.push({name,passed:!!passed});if(!passed)throw Error(name);}
 async function focus(element,text,start=0,end=start){element.value=text;element.focus();element.setSelectionRange(start,end);await delay(180);}
 const capture=()=>call('capture');
 const deliver=(target,text)=>call('deliver',{target,text});
 async function rejected(task){try{await task;return false;}catch{return true;}}
 try{
  await delay(800);
  await focus(one,'前😀后',1,3);
  let target=await capture();
  check('webview_uses_generic_uia',target.input_adapter==='uia_value'&&target.delivery_supported===true);
  check('explicit_selection_only_in_json',target.selection==='😀'&&!Object.hasOwn(target,'text'));
  check('unicode_selected_replacement',(await deliver(target,'新词')).verified===true&&one.value==='前新词后');
  check('caret_after_replacement',one.selectionStart===3&&one.selectionEnd===3);
  check('web_app_receives_input_event',inputs===1&&model===one.value);
  check('one_shot_token',await rejected(deliver(target,'NO')));
  await focus(one,'baseline',3);target=await capture();one.value='changed';
  check('changed_content_rejected',await rejected(deliver(target,'NO'))&&one.value==='changed');
  await focus(one,'baseline',3);target=await capture();one.setSelectionRange(1,1);
  check('changed_caret_rejected',await rejected(deliver(target,'NO'))&&one.value==='baseline');
  await focus(one,'field A',0);target=await capture();await focus(two,'field B',0);
  check('different_field_rejected',await rejected(deliver(target,'NO'))&&one.value==='field A'&&two.value==='field B');
  await focus(one,'A',1);const stale=await capture();await focus(two,'B',1);target=await capture();
  check('new_capture_invalidates_old',await rejected(deliver(stale,'NO')));
  check('second_field_receives',(await deliver(target,'2')).verified===true&&two.value==='B2'&&one.value==='A');
  await focus(one,'A',1);target=await capture();
  check('return_to_first_field_is_fresh',(await deliver(target,'1')).verified===true&&one.value==='A1'&&two.value==='B2');
  await focus(one,'keep selection',0,4);target=await capture();const before=inputs;
  check('question_restore_does_not_replace',(await call('restore',{target})).ok===true&&one.value==='keep selection'&&one.selectionStart===0&&one.selectionEnd===4&&inputs===before);
  await focus(one,'unchanged',0);target=await capture();
  check('multiline_not_stripped_by_single_line',await rejected(deliver(target,'line1\nline2'))&&one.value==='unchanged');
  await focus(one,'unchanged',0);target=await capture();
  check('nul_rejected',await rejected(deliver(target,'\0'))&&one.value==='unchanged');
  await focus(one,'unchanged',0);target=await capture();one.readOnly=true;
  check('became_readonly_rejected',await rejected(deliver(target,'NO'))&&one.value==='unchanged');one.readOnly=false;
  await focus(one,'unchanged',0);target=await capture();one.disabled=true;
  check('became_disabled_rejected',await rejected(deliver(target,'NO'))&&one.value==='unchanged');one.disabled=false;
  for(const id of ['password','readonly']){
   const field=document.querySelector('#'+id);field.focus();await delay(150);
   check(id+'_capture_rejected',await rejected(capture()));
  }
  await focus(one,'A',1);const old=await capture();document.querySelector('#password').focus();await delay(150);
  check('unsupported_capture_fails',await rejected(capture()));
  check('unsupported_capture_clears_old_token',await rejected(deliver(old,'NO'))&&one.value==='A');
  await focus(one,'x'.repeat(256*1024+1),0);
  // Allow the renderer to publish the large programmatically seeded value.
  // Production delivery independently checks the current value before writing.
  await delay(1000);
  check('oversized_fixture_exceeds_bound',one.value.length>256*1024);
  check('oversized_field_rejected',await rejected(capture()));
  await focus(one,'中'.repeat(4000),0);await delay(500);
  check('truncated_multibyte_value_rejected',await rejected(capture())&&one.value.length===4000);
  await focus(one,'front😀e\u0301back',9);target=await capture();
  check('emoji_and_combining_prefix_caret',(await deliver(target,'中')).verified===true&&one.value==='front😀e\u0301中back'&&one.selectionStart===10);
  await focus(one,'',0);target=await capture();const emptyInputs=inputs;
  check('empty_field_receives',(await deliver(target,'空白输入')).verified===true&&one.value==='空白输入');
  check('empty_field_caret_and_one_event',one.selectionStart===4&&one.selectionEnd===4&&inputs===emptyInputs+1);
  check('never_enter_or_submit',enters===0&&submits===0);
  await call('report',{passed:true,kind:'native WebView2 UI Automation receipt; synthetic fields only',checks,audio_captured:false,models_contacted:false});
  document.querySelector('#status').textContent='Passed';
 }catch(error){
  await call('report',{passed:false,kind:'native WebView2 UI Automation receipt; synthetic fields only',checks,error:String(error),audio_captured:false,models_contacted:false});
  document.querySelector('#status').textContent='Failed: '+String(error);
 }
})().catch(()=>{});
