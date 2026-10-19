"""Generate notices from locked macOS dependencies. Missing crate texts use pinned VCS."""
from pathlib import Path
import concurrent.futures, hashlib, json, shutil, subprocess, urllib.request
root=Path(__file__).resolve().parent.parent
meta=json.loads(subprocess.check_output(['cargo','metadata','--offline','--locked','--format-version','1','--filter-platform','aarch64-apple-darwin'],cwd=root))
ids={x['id'] for x in meta['resolve']['nodes']}
out=root/'licenses'
if out.exists():shutil.rmtree(out)
out.mkdir()
def collect(p):
 src=Path(p['manifest_path']).parent;dest=out/(p['name']+'-'+p['version']);dest.mkdir()
 files=[f for f in src.rglob('*') if f.is_file() and f.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','COPYRIGHT')) and not any(x in f.parts for x in ('test','tests','benches','examples'))]
 if p.get('license_file') and (src/p['license_file']).is_file():files.append(src/p['license_file'])
 refs=[]
 for f in dict.fromkeys(files):
  target=dest/str(f.relative_to(src)).replace('/','__');shutil.copy2(f,target)
  refs.append({'file':target.name,'sha256':hashlib.sha256(target.read_bytes()).hexdigest(),'source':'crate archive'})
 if not refs:
  vcs=json.loads((src/'.cargo_vcs_info.json').read_text());sha=vcs['git']['sha1'];repo=p['repository'].removesuffix('/').removesuffix('.git').replace('https://github.com/','')
  # Workspace license files are often omitted from published subcrates.
  for name in ['LICENSE.md','LICENSE.txt','LICENSE','LICENSE-MIT','LICENSE-APACHE']:
   url=f'https://raw.githubusercontent.com/{repo}/{sha}/{name}'
   try:
    content=urllib.request.urlopen(url,timeout=20).read()
   except Exception:continue
   if len(content)<100:continue
   target=dest/name;target.write_bytes(content);refs.append({'file':name,'sha256':hashlib.sha256(content).hexdigest(),'source':url})
  if not refs and p['license']=='MPL-2.0':
   url='https://www.mozilla.org/media/MPL/2.0/index.txt'
   content=urllib.request.urlopen(url,timeout=20).read()
   if b'Mozilla Public License' in content:
    target=dest/'LICENSE-MPL-2.0';target.write_bytes(content);refs.append({'file':target.name,'sha256':hashlib.sha256(content).hexdigest(),'source':url})
  if not refs:raise RuntimeError('Missing upstream license text: '+p['name'])
 return {'name':p['name'],'version':p['version'],'license':p['license'],'repository':p['repository'],'authors':p['authors'],'files':refs}
packages=sorted([p for p in meta['packages'] if p['id'] in ids and p['source']],key=lambda x:(x['name'],x['version']))
with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:entries=list(pool.map(collect,packages))
(out/'index.json').write_text(json.dumps(entries,ensure_ascii=False,indent=2)+'\n')
(root/'THIRD_PARTY_NOTICES.md').write_text('# Third-party notices\n\nGenerated from Cargo.lock for aarch64-apple-darwin, including build dependencies. Full license texts, authors and pinned upstream sources are in licenses/. System frameworks (AppKit, WebKit, CoreAudio, Security) are provided by macOS and are not redistributed. Vao source and original brand assets are AGPL-3.0-only; see LICENSE and PROVENANCE.md.\n\n| Package | Version | License |\n| --- | --- | --- |\n'+''.join(f"| {p['name']} | {p['version']} | {p['license']} |\n" for p in entries))
print(json.dumps({'packages':len(entries),'missing_license_texts':0}))
