// Characterize real UIA providers using owned synthetic fields. A completed
// observation is not a claim that that field shape supports delivery.
export async function characterize({call,check,delay}){
 const observations=[];
 const style=document.createElement('style');
 style.textContent='[data-uia-placeholder]:empty::before{content:attr(data-uia-placeholder);pointer-events:none}';
 document.head.append(style);
 const cases=[
  {id:'input_placeholder',tag:'input',placeholder:'提示文字呀'},
  {id:'textarea_placeholder',tag:'textarea',placeholder:'提示文字呀'},
  {id:'editable_empty',tag:'div',html:''},
  {id:'editable_paragraph_br',tag:'div',html:'<p><br></p>'},
  {id:'editable_generated_placeholder',tag:'div',html:'',generated:true},
  {id:'editable_existing_text',tag:'div',html:'原文',offset:1}
 ];
 const inserted='中🙂';
 for(const item of cases){
  const field=document.createElement(item.tag);
  if(item.tag==='div'){
   field.contentEditable='true';field.setAttribute('role','textbox');
   field.setAttribute('aria-multiline','true');field.innerHTML=item.html;
   if(item.generated)field.dataset.uiaPlaceholder='提示文字呀';
  }else field.placeholder=item.placeholder;
  field.setAttribute('aria-label','Synthetic '+item.id);
  document.querySelector('#form').append(field);
  let inputs=0;field.addEventListener('input',()=>inputs++);
  field.focus();
  if(item.tag==='div'){
   const range=document.createRange();
   if(item.offset)range.setStart(field.firstChild,item.offset);
   else range.setStart(field.firstChild?.firstChild||field,0);
   range.collapse(true);window.getSelection().removeAllRanges();window.getSelection().addRange(range);
  }else field.setSelectionRange(0,0);
  await delay(220);
  const read=()=>item.tag==='div'?field.textContent:field.value;
  const original=read(),expected=item.offset?'原'+inserted+'文':inserted;
  const observation={shape:item.id,capture_supported:false,receipt_verified:false,text_received_exactly:false,warning_after_exact_input:false};
  let target;
  try{target=await call('capture');observation.capture_supported=true;}catch{}
  if(target){
   try{observation.receipt_verified=(await call('deliver',{target,text:inserted})).verified===true;}catch{}
   observation.text_received_exactly=read()===expected;
   observation.warning_after_exact_input=!observation.receipt_verified&&observation.text_received_exactly;
   if(observation.receipt_verified)check('shape_'+item.id+'_verified_receipt_is_exact',observation.text_received_exactly);
   const prior=read(),count=inputs;
   let replayRejected=false;try{await call('deliver',{target,text:'NO'});}catch{replayRejected=true;}
   check('shape_'+item.id+'_does_not_replay',replayRejected&&read()===prior&&inputs===count);
  }else check('shape_'+item.id+'_capture_refusal_sends_nothing',read()===original&&inputs===0);
  observations.push(observation);field.remove();
 }
 style.remove();
 return observations;
}
