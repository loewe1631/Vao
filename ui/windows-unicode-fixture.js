// Native, opt-in Unicode transport tests. All fields contain synthetic text.
const invoke=window.__TAURI__.core.invoke;
const checks=[];
let observations=[];
let enters=0,submits=0,inputs=0,trustedInputs=0,model='';
const one=document.querySelector('#one'),two=document.querySelector('#two');
document.addEventListener('keydown',e=>{if(e.key==='Enter')enters++;});
document.querySelector('#form').addEventListener('submit',e=>{e.preventDefault();submits++;});
one.addEventListener('input',e=>{inputs++;if(e.isTrusted)trustedInputs++;model=one.value;});
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
 check('unicode_uses_verified_uia_snapshot',target.input_adapter==='uia_unicode'&&target.delivery_supported===true);
 check('unicode_explicit_selection_only_in_json',target.selection==='😀'&&!Object.hasOwn(target,'text'));
 check('unicode_selected_replacement',(await deliver(target,'新词🙂')).verified===true&&one.value==='前新词🙂后');
 check('unicode_caret_after_replacement',one.selectionStart===5&&one.selectionEnd===5);
 check('unicode_trusted_input_updates_web_model',trustedInputs>0&&trustedInputs===inputs&&model===one.value);
 // Exercise the browser's actual undo history, not SetValue followed by a
 // synthetic caret move. One undo must restore the whole selected phrase.
 check('unicode_one_undo_restores_selection_text',document.execCommand('undo')&&one.value==='前😀后');
 check('unicode_redo_restores_inserted_text',document.execCommand('redo')&&one.value==='前新词🙂后');
 check('unicode_one_shot_token',await rejected(deliver(target,'NO')));
 await focus(one,'baseline',3);target=await capture();one.value='changed';await delay(180);
 let before=inputs;
 check('unicode_changed_content_rejected',await rejected(deliver(target,'NO'))&&one.value==='changed'&&inputs===before);
 await focus(one,'baseline',3);target=await capture();one.setSelectionRange(1,1);await delay(180);
 check('unicode_changed_caret_rejected',await rejected(deliver(target,'NO'))&&one.value==='baseline');
 await focus(one,'baseline',1,4);target=await capture();one.setSelectionRange(1,5);await delay(180);
 check('unicode_changed_selection_rejected',await rejected(deliver(target,'NO'))&&one.value==='baseline');
 await focus(one,'field A',0);target=await capture();await focus(two,'field B',0);
 check('unicode_different_field_rejected',await rejected(deliver(target,'NO'))&&one.value==='field A'&&two.value==='field B');
 await focus(one,'A',1);const stale=await capture();await focus(two,'B',1);target=await capture();
 check('unicode_new_capture_invalidates_old',await rejected(deliver(stale,'NO')));
 check('unicode_second_field_receives',(await deliver(target,'2')).verified===true&&two.value==='B2'&&one.value==='A');
 await focus(one,'A',1);target=await capture();
 check('unicode_return_to_first_field_is_fresh',(await deliver(target,'1')).verified===true&&one.value==='A1'&&two.value==='B2');
 await focus(one,'keep selection',0,4);target=await capture();before=inputs;
 check('unicode_question_restore_does_not_replace',(await call('restore',{target})).ok===true&&one.value==='keep selection'&&one.selectionStart===0&&one.selectionEnd===4&&inputs===before);
 for(const [label,text] of [['lf','line1\nline2'],['cr','line1\rline2'],['tab','A\tB'],['escape','A\u001bB'],['line_separator','A\u2028B'],['nul','\0']]){
  await focus(one,'unchanged',0);target=await capture();before=inputs;
  check('unicode_'+label+'_rejected_without_input',await rejected(deliver(target,text))&&one.value==='unchanged'&&inputs===before);
 }
 const many=document.querySelector('#many');
 await focus(many,'unchanged',0);target=await capture();
 check('unicode_multiline_does_not_guess_enter_semantics',await rejected(deliver(target,'line1\nline2'))&&many.value==='unchanged');
 await focus(one,'unchanged',0);target=await capture();one.readOnly=true;await delay(180);
 check('unicode_became_readonly_rejected',await rejected(deliver(target,'NO'))&&one.value==='unchanged');one.readOnly=false;
 await focus(one,'unchanged',0);target=await capture();one.disabled=true;await delay(180);
 check('unicode_became_disabled_rejected',await rejected(deliver(target,'NO'))&&one.value==='unchanged');one.disabled=false;
 for(const id of ['password','readonly']){
  document.querySelector('#'+id).focus();await delay(180);
  check('unicode_'+id+'_capture_rejected',await rejected(capture()));
 }
 await focus(one,'A',1);const old=await capture();document.querySelector('#password').focus();await delay(180);
 check('unicode_unsupported_capture_fails',await rejected(capture()));
 check('unicode_unsupported_capture_clears_token',await rejected(deliver(old,'NO'))&&one.value==='A');
 await focus(one,'中'.repeat(4000),0);await delay(500);
 check('unicode_truncated_multibyte_field_rejected',await rejected(capture())&&one.value.length===4000);
 await focus(one,'front😀e\u0301back',9);target=await capture();
 check('unicode_emoji_and_combining_caret',(await deliver(target,'中🙂')).verified===true&&one.value==='front😀e\u0301中🙂back'&&one.selectionStart===12);
 await focus(one,'',0);target=await capture();
 check('unicode_empty_field_receives',(await deliver(target,'空白输入')).verified===true&&one.value==='空白输入'&&one.selectionStart===4&&model===one.value);
 await focus(one,'original',8);target=await capture();
 await call('cover');await delay(250);
 check('unicode_returns_from_other_window',(await deliver(target,'恢复')).verified===true&&one.value==='original恢复'&&document.hasFocus());
 // A web editor may normalize its own empty line after an input event. The
 // transport must report an unconfirmed receipt and consume the token even
 // when the inserted phrase is visible; it must never resend automatically.
 await focus(many,'\n',0);target=await capture();
 const normalize=()=>{many.value=many.value.replaceAll('\n','');};
 many.addEventListener('input',normalize);
 let normalizedError='';try{await deliver(target,'已输入🙂');}catch(error){normalizedError=String(error);}
 many.removeEventListener('input',normalize);
 check('unicode_normalized_field_received_phrase',many.value==='已输入🙂');
 check('unicode_normalized_field_keeps_receipt_warning',normalizedError.includes('Receipt not confirmed'));
 check('unicode_normalized_field_token_cannot_replay',await rejected(deliver(target,'NO'))&&many.value==='已输入🙂');
 await focus(many,'\n',1);target=await capture();
 many.addEventListener('input',normalize);
 let endLfError='';try{await deliver(target,'末尾🙂');}catch(error){endLfError=String(error);}
 many.removeEventListener('input',normalize);
 check('unicode_real_lf_at_end_is_not_empty_paragraph',endLfError.includes('Receipt not confirmed')&&many.value==='末尾🙂');
 // The accessible-name exception is for an empty Value only, and is part of
 // the snapshot: neither actual text nor a changed representation is ignored.
 one.setAttribute('aria-label','Synthetic empty name A');
 await focus(one,'',0);target=await capture();one.value='actual';await delay(180);
 check('unicode_empty_name_content_change_rejected',await rejected(deliver(target,'NO'))&&one.value==='actual');
 await focus(one,'',0);target=await capture();one.setAttribute('aria-label','Synthetic empty name B');await delay(180);
 check('unicode_empty_name_change_rejected',await rejected(deliver(target,'NO'))&&one.value==='');
 await focus(one,'Synthetic empty name B',0);target=await capture();
 check('unicode_nonempty_name_is_preserved',(await deliver(target,'中')).verified===true&&one.value==='中Synthetic empty name B');
 one.removeAttribute('aria-label');
 const {characterize}=await import('./windows-uia-shapes.js');
 observations=await characterize({call,check,delay});
 check('unicode_six_field_shapes_observed',observations.length===6);
 check('unicode_never_enter_or_submit',enters===0&&submits===0);
 await call('report',{passed:true,kind:'guarded Unicode; native WebView2; synthetic fields only',checks,observations,audio_captured:false,models_contacted:false});
 document.querySelector('#status').textContent='Passed';
}catch(error){
 await call('report',{passed:false,kind:'guarded Unicode; native WebView2; synthetic fields only',checks,observations,error:String(error),audio_captured:false,models_contacted:false});
 document.querySelector('#status').textContent='Failed: '+String(error);
}
