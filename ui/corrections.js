'use strict';
/* Local, explicitly confirmed corrections. No model requests and no timers. */
window.createCorrections = function ({ipc,getData,getSnap,render,refresh,poll,toast,esc}) {
  let tab='words',query='',selectedGroups=new Set(),promptKey='',promptSelected=new Set(),busy=false;
  const en=()=>getData().settings.ui_language==='en';
  const l=(zh,english)=>en()?english:zh;
  const book=()=>getData().corrections||{revision:0,enabled:true,groups:[]};
  const effective=r=>r?.effective_text??r?.text??'';
  const buttons=(id)=>`<button data-correct-result="${id}">${l('纠正文字','Correct text')}</button>`;
  const tabs=()=>`<div class="correction-tabs" role="tablist"><button role="tab" data-correction-tab="words" aria-selected="${tab==='words'}">${l('词语提示','Word hints')}</button><button role="tab" data-correction-tab="corrections" aria-selected="${tab==='corrections'}">${l('个人纠错','Personal corrections')}</button></div>`;
  function library(original) {
    if(tab==='words')return tabs()+original;
    const b=book();selectedGroups=new Set([...selectedGroups].filter(correct=>b.groups.some(g=>g.correct===correct)));const groups=b.groups.map((g,i)=>({g,i})).filter(({g})=>[g.correct,...g.aliases.map(a=>a.wrong)].some(t=>t.toLowerCase().includes(query.toLowerCase())));
    return `${tabs()}<h1>${l('个人纠错','Personal corrections')}</h1><p>${l('只记住你确认的错误写法。下次整理后遇到它，先询问，再输入。','Remember only changes you confirm. When a saved phrase appears after processing, review it before inserting.')}</p><div class="card"><div class="row between"><div><h2>${l('纠错建议','Correction suggestions')}</h2><p class="muted">${l('本机匹配，不自动替换，不增加模型调用。','Matched on this device. No automatic replacement or extra model call.')}</p></div><button id="correction-enabled" class="switch" role="switch" aria-checked="${b.enabled}"></button></div><div class="correction-tools"><button id="correction-add" class="primary">${l('添加纠错','Add correction')}</button><button id="correction-import">${l('导入 JSON','Import JSON')}</button><button id="correction-export">${l('全部导出','Export all')}</button><button id="correction-export-selected" ${selectedGroups.size?'':'disabled'}>${l('导出所选','Export selected')}</button><button id="correction-undo">${l('撤销最近变更','Undo last change')}</button></div><label class="field">${l('搜索正确或错误写法','Search correct or mistaken phrase')}<input id="correction-search" value="${esc(query)}" autocomplete="off"></label><p class="muted">${b.groups.length} ${l('组','groups')} · ${b.groups.reduce((n,g)=>n+g.aliases.length,0)} ${l('个错误写法','aliases')}</p>${groups.map(({g,i})=>`<article class="correction-group"><div class="row between"><label class="correction-select"><input type="checkbox" data-group-select="${i}" ${selectedGroups.has(g.correct)?'checked':''}><strong>${esc(g.correct)}</strong></label><div class="row"><button data-group-toggle="${i}" aria-pressed="${g.enabled}">${g.enabled?l('已启用','Enabled'):l('已停用','Disabled')}</button><button data-group-edit="${i}">${l('编辑','Edit')}</button><button data-group-delete="${i}" class="quiet danger">${l('删除','Delete')}</button></div></div><div class="correction-aliases">${g.aliases.map((a,j)=>`<label><input type="checkbox" data-alias-toggle="${i}:${j}" ${a.enabled?'checked':''}>${esc(a.wrong)} <span aria-hidden="true">→</span> ${esc(g.correct)}</label>`).join('')}</div></article>`).join('')||`<div class="empty">${l('没有匹配的纠错记录。可从结果或历史点“纠正文字”开始。','No matching corrections. Start with “Correct text” on a result or in History.')}</div>`}</div><p class="muted">${l('词库仅存本机。导出文件包含个人词语，请自行保管。清空历史不会删除词库。','This library stays on your device. Exported files contain your personal phrases. Clearing History keeps the library.')}</p>`;
  }
  function modal(title,body,bind) {
    document.querySelector('#correction-dialog')?.remove();
    const root=document.createElement('div');root.id='correction-dialog';root.className='correction-modal';
    root.innerHTML=`<section role="dialog" aria-modal="true" aria-labelledby="correction-dialog-title" tabindex="-1"><div class="row between"><h2 id="correction-dialog-title">${esc(title)}</h2><button data-modal-close aria-label="${l('关闭','Close')}">×</button></div>${body}<p class="correction-error" role="alert"></p></section>`;
    document.body.append(root);const before=document.activeElement;
    const close=()=>{root.remove();document.removeEventListener('keydown',key,true);before?.focus?.();};
    const key=e=>{if(e.key==='Escape'){e.preventDefault();e.stopPropagation();close();}if(e.key==='Tab'){const all=[...root.querySelectorAll('button,input,textarea,select')].filter(e=>!e.disabled);if(!all.length)return;if(e.shiftKey&&document.activeElement===all[0]){e.preventDefault();all.at(-1).focus();}else if(!e.shiftKey&&document.activeElement===all.at(-1)){e.preventDefault();all[0].focus();}}};
    root.querySelector('[data-modal-close]').onclick=close;document.addEventListener('keydown',key,true);
    const run=fn=>async e=>{const button=e?.currentTarget;if(button)button.disabled=true;try{await fn();}catch(error){root.querySelector('.correction-error').textContent=String(error);}finally{if(button)button.disabled=false;}};
    bind(root,close,run);root.querySelector('input,textarea,select,section').focus();return {root,close};
  }
  async function save(b){await ipc('save_correction_book',{book:b,revision:book().revision});await refresh();render();}
  function groupEditor(index) {
    const source=book(),g=index==null?null:source.groups[index];
    modal(l('编辑纠错组','Edit correction group'),`<label class="field">${l('正确写法（相同写法自动归组）','Correct phrase (identical phrases merge)')}<input id="group-correct" value="${esc(g?.correct||'')}" maxlength="240"></label><label class="field">${l('错误写法，每行一个','Mistaken phrases, one per line')}<textarea id="group-wrong" rows="6">${esc(g?.aliases.map(a=>a.wrong).join('\n')||'')}</textarea></label><button id="group-save" class="primary">${l('保存','Save')}</button>`,(root,close,run)=>{
      root.querySelector('#group-save').onclick=run(async()=>{
        const correct=root.querySelector('#group-correct').value.trim(),aliases=[...new Set(root.querySelector('#group-wrong').value.split(/\r?\n/).map(s=>s.trim()).filter(Boolean))];
        if(!correct||!aliases.length)throw l('请填写正确写法与至少一个错误写法。','Enter a correct phrase and at least one mistaken phrase.');
        const b=structuredClone(source);if(g)b.groups.splice(index,1);
        let target=b.groups.find(x=>x.correct===correct);if(!target){target={correct,enabled:g?.enabled??true,aliases:[]};b.groups.push(target);}
        for(const wrong of aliases){if(!target.aliases.some(a=>a.wrong===wrong))target.aliases.push({wrong,enabled:g?.aliases.find(a=>a.wrong===wrong)?.enabled??true});}
        await save(b);close();
      });
    });
  }
  function proposedPair(a,b) {
    let start=0;while(start<a.length&&start<b.length&&a[start]===b[start])start++;
    let endA=a.length,endB=b.length;while(endA>start&&endB>start&&a[endA-1]===b[endB-1]){endA--;endB--;}
    // Extend a single changed span to whole words; never learn this without a checked box.
    while(start>0&&/[A-Za-z0-9_]/.test(a[start-1])&&/[A-Za-z0-9_]/.test(b[start-1]))start--;
    while(endA<a.length&&/[A-Za-z0-9_]/.test(a[endA]))endA++;
    while(endB<b.length&&/[A-Za-z0-9_]/.test(b[endB]))endB++;
    const wrong=a.slice(start,endA),correct=b.slice(start,endB);
    return wrong&&correct&&wrong!==correct&&wrong.length<=160&&correct.length<=160&&!/[\n\r\u3400-\u9fff]/.test(wrong+correct)?{wrong,correct}:{wrong:'',correct:''};
  }
  function editResult(id) {
    const snap=getSnap(),r=(snap.result?.id===id?snap.result:getData().history.find(r=>r.id===id));if(!r)return;
    const expected=effective(r),sessionId=snap.session.id;
    modal(l('纠正文字','Correct text'),`<p class="muted">${l('保存只修改 Vao 中的纠正版，不会追改已输入到其他应用的文字。','Saving updates this result in Vao. Text already inserted in other apps stays as it is.')}</p><label class="field">${l('纠正版','Corrected text')}<textarea id="corrected-result" rows="6">${esc(expected)}</textarea></label><div class="row between"><h3>${l('可选：记住错误写法','Optional: remember mistaken phrases')}</h3><button id="correction-pair-add">${l('添加一项','Add a phrase')}</button></div><p class="muted">${l('只勾选你确认的词语对应关系。中文请填写完整词语，避免只记住单字差异。','Check only phrase pairs you confirm. Enter complete phrases rather than single-character differences.')}</p><div id="correction-pairs"></div><button id="result-correction-save" class="primary">${l('保存纠正','Save correction')}</button>`,(root,close,run)=>{
      const rows=root.querySelector('#correction-pairs');let untouched=true;
      const add=({wrong='',correct=''}={})=>{const row=document.createElement('div');row.className='correction-pair';row.innerHTML=`<input type="checkbox" aria-label="${l('记住此对应关系','Remember this pair')}"><input data-wrong aria-label="${l('错误写法','Mistaken phrase')}" placeholder="${l('错误写法','Mistaken phrase')}" value="${esc(wrong)}"><span>→</span><input data-correct aria-label="${l('正确写法','Correct phrase')}" placeholder="${l('正确写法','Correct phrase')}" value="${esc(correct)}"><button aria-label="${l('移除','Remove')}">×</button>`;row.querySelector('button').onclick=()=>row.remove();row.addEventListener('input',()=>untouched=false);rows.append(row);};
      add();root.querySelector('#correction-pair-add').onclick=()=>{untouched=false;add();};
      root.querySelector('#corrected-result').oninput=e=>{if(untouched&&rows.children.length===1){const p=proposedPair(expected,e.target.value);rows.querySelector('[data-wrong]').value=p.wrong;rows.querySelector('[data-correct]').value=p.correct;}};
      root.querySelector('#result-correction-save').onclick=run(async()=>{
        const text=root.querySelector('#corrected-result').value,pairs=[...rows.children].filter(row=>row.querySelector('[type=checkbox]').checked).map(row=>({wrong:row.querySelector('[data-wrong]').value.trim(),correct:row.querySelector('[data-correct]').value.trim()}));
        if(pairs.some(p=>!p.wrong||!p.correct||!expected.includes(p.wrong)||!text.includes(p.correct)))throw l('请核对勾选的错误写法与正确写法确实在本次修改中。','Each checked pair must occur in the original and corrected text.');
        await ipc('save_result_correction',{id,sessionId,revision:r.edit_revision||0,expectedText:expected,text,pairs});close();await refresh();await poll();render();toast(l('已保存纠正','Correction saved'));
      });
    });
  }
  async function importBook(){
    const preview=await ipc('preview_correction_import');if(preview?.cancelled)return;
    const conflicts=new Map();for(const c of preview.conflicts){if(!conflicts.has(c.key))conflicts.set(c.key,{wrong:c.wrong,targets:new Set()});for(const value of [c.existing,c.incoming])if(value)conflicts.get(c.key).targets.add(value);}
    const fields=[...conflicts].map(([key,c],i)=>({key,...c,i}));let committed=false;
    const dialog=modal(l('确认导入','Confirm import'),`<p>${l('预计新增','Estimated additions')}: ${preview.added_groups} ${l('组','groups')} / ${preview.added_aliases} ${l('个错误写法','aliases')} · ${l('重复','Duplicates')}: ${preview.duplicates} · ${l('跳过','Skipped')}: ${preview.skipped}</p><p class="muted">${l('只合并词库，不包含历史、录音、账号或密钥。冲突选择会调整上述数量。','Merges phrases only. No history, audio, accounts or keys. Conflict choices can change these counts.')}</p>${fields.map(c=>`<label class="field">${esc(c.wrong)}<select data-import-choice="${c.i}"><option value="__unselected">${l('请选择如何处理冲突','Choose how to resolve this conflict')}</option><option value="">${l('跳过此错误写法，保留本机记录','Skip alias and keep local record')}</option>${[...c.targets].map(t=>`<option value="${esc(t)}">${esc(t)}</option>`).join('')}</select></label>`).join('')}<button id="correction-import-confirm" class="primary">${l('确认合并','Confirm merge')}</button>`,(root,close,run)=>{
      root.querySelector('#correction-import-confirm').onclick=run(async()=>{const choices={};for(const c of fields){const value=root.querySelector(`[data-import-choice="${c.i}"]`).value;if(value==='__unselected')throw l('请确认每一项冲突。','Resolve every conflict.');choices[c.key]=value;}await ipc('commit_correction_import',{token:preview.token,choices});committed=true;close();await refresh();render();toast(l('词库已合并','Library merged'));});
    });
    const observer=new MutationObserver(()=>{if(!document.contains(dialog.root)){observer.disconnect();if(!committed)ipc('cancel_correction_import',{token:preview.token}).catch(()=>{});}});observer.observe(document.body,{childList:true});
  }
  function updatePrompt() {
    const snap=getSnap(),p=snap.session.phase==='awaiting_correction'?snap.correction:null;
    const signature=p?JSON.stringify([p.session_id,p.items,en()]):'';
    const containers=document.querySelectorAll('[data-correction-prompt]');
    if(!p){promptKey='';containers.forEach(e=>{e.innerHTML='';e.classList.add('hidden');});return;}
    if(signature!==promptKey){promptKey=signature;promptSelected=new Set(p.items.length===1?[p.items[0].id]:[]);busy=false;}
    for(const container of containers){container.classList.remove('hidden');if(container.dataset.signature===signature)continue;container.dataset.signature=signature;
      container.innerHTML=`<section class="correction-review"><div class="correction-review-head" data-bar-drag><strong>${l('是否使用你记住的写法？','Use a phrase you remembered?')}</strong><span>${l('等待确认','Awaiting your choice')}</span></div><div class="correction-review-items">${p.items.map(item=>`<label class="correction-candidate"><input type="checkbox" data-candidate="${item.id}" ${promptSelected.has(item.id)?'checked':''}><div><strong>${esc(item.found)} → ${esc(item.correct)}</strong><p>${esc(item.before)}<mark>${esc(item.found)}</mark>${esc(item.after)}</p></div></label>`).join('')}</div><div class="correction-review-actions"><button data-correction-apply class="primary" ${promptSelected.size?'':'disabled'}>${p.auto_input?l('替换并输入','Replace and insert'):l('替换并保存','Replace and save')}</button><button data-correction-keep>${p.auto_input?l('保留原文并输入','Keep and insert'):l('保留原文','Keep original')}</button><button data-correction-cancel>${l('取消输入','Cancel')}</button></div><p class="correction-review-note">${l('未勾选的位置保留原文。不会自动发送。','Unchecked phrases stay as they are. Nothing is sent automatically.')} · ${l('词库快照','Library snapshot')} ${p.book_revision??0}</p><p class="correction-error" role="alert"></p></section>`;
    }
  }
  document.addEventListener('input',e=>{if(e.target.id==='correction-search'){query=e.target.value;const at=e.target.selectionStart;render();const input=document.querySelector('#correction-search');input?.focus();input?.setSelectionRange(at,at);}});
  document.addEventListener('change',async e=>{
    const el=e.target;try{
      if(el.matches('[data-candidate]')){const item=getSnap().correction?.items.find(i=>i.id===+el.dataset.candidate);if(el.checked){for(const id of item?.overlaps||[])promptSelected.delete(id);promptSelected.add(+el.dataset.candidate);}else promptSelected.delete(+el.dataset.candidate);document.querySelectorAll('[data-candidate]').forEach(c=>c.checked=promptSelected.has(+c.dataset.candidate));document.querySelectorAll('[data-correction-apply]').forEach(b=>b.disabled=!promptSelected.size);}
      if(el.matches('[data-group-select]')){const g=book().groups[+el.dataset.groupSelect];el.checked?selectedGroups.add(g.correct):selectedGroups.delete(g.correct);document.querySelector('#correction-export-selected').disabled=!selectedGroups.size;}
      if(el.matches('[data-alias-toggle]')){const [i,j]=el.dataset.aliasToggle.split(':').map(Number),b=structuredClone(book());b.groups[i].aliases[j].enabled=el.checked;await save(b);}
    }catch(error){toast(error);await refresh();render();}
  });
  document.addEventListener('click',async e=>{
    const el=e.target.closest('button');if(!el)return;try{
      if(el.matches('[data-correction-tab]')){tab=el.dataset.correctionTab;render();}
      if(el.matches('[data-correct-result]'))editResult(+el.dataset.correctResult);
      if(el.id==='correction-add')groupEditor(null);
      if(el.matches('[data-group-edit]'))groupEditor(+el.dataset.groupEdit);
      if(el.id==='correction-enabled'){const b=structuredClone(book());b.enabled=!b.enabled;await save(b);}
      if(el.matches('[data-group-toggle]')){const b=structuredClone(book());b.groups[+el.dataset.groupToggle].enabled=!b.groups[+el.dataset.groupToggle].enabled;await save(b);}
      if(el.matches('[data-group-delete]')){const i=+el.dataset.groupDelete,g=book().groups[i];modal(l('删除纠错组','Delete correction group'),`<p>${esc(g.correct)}</p><p>${l('删除本组的所有错误写法，可撤销最近一次变更。','Remove all aliases in this group. The last change can be undone.')}</p><button id="correction-delete-confirm" class="danger">${l('确认删除','Delete group')}</button>`,(root,close,run)=>{root.querySelector('#correction-delete-confirm').onclick=run(async()=>{const b=structuredClone(book());b.groups.splice(i,1);await save(b);selectedGroups.delete(g.correct);close();});});}
      if(el.id==='correction-undo'){await ipc('undo_correction_book',{revision:book().revision});await refresh();render();}
      if(el.id==='correction-import')await importBook();
      if(el.id==='correction-export'||el.id==='correction-export-selected'){const out=await ipc('export_corrections',{selected:el.id.endsWith('-selected')?[...selectedGroups]:null});if(!out?.cancelled)toast(l('已导出词库','Library exported'));}
      if(el.matches('[data-correction-apply],[data-correction-keep],[data-correction-cancel]')){
        if(busy)return;const p=getSnap().correction;if(!p)return;busy=true;document.querySelectorAll('.correction-review button').forEach(b=>b.disabled=true);
        try{if(el.hasAttribute('data-correction-cancel'))await ipc('cancel');else await ipc('resolve_correction_prompt',{sessionId:p.session_id,selected:el.hasAttribute('data-correction-keep')?[]:[...promptSelected]});await poll();}
        catch(error){const target=document.querySelector('.correction-review .correction-error');if(target)target.textContent=String(error);throw error;}
        finally{busy=false;document.querySelectorAll('.correction-review button').forEach(b=>b.disabled=b.hasAttribute('data-correction-apply')&&!promptSelected.size);}
      }
    }catch(error){toast(error);}
  });
  return {library,effective,buttons,updatePrompt};
};
