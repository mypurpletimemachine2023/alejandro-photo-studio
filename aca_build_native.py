"""Reproducible native web release: pinned dependencies, bounded final-web codegen.
The final crate alone receives thin-LTO/codegen-units=16; dependency cache/profile
remain unchanged. Standard wasm-bindgen output and source HTML/assets are packaged.
"""
from pathlib import Path
from html.parser import HTMLParser
import argparse, hashlib, json, os, re, shutil, subprocess, time
BASE=Path(__file__).resolve().parent.parent
ROOT=BASE/'photocraft'
TOOL=BASE/'.toolchain'; TARGET=BASE/'.target-web'
CARGO=TOOL/'cargo/bin/cargo.exe'
BINDGEN=TOOL/'tools/wasm-bindgen-0.2.129-x86_64-pc-windows-msvc/wasm-bindgen.exe'
OPT=TOOL/'tools/wasm-opt.exe'
ROWS={'photo':'photocraft','vector':'vectorcraft','motion':'effectcraft','film':'filmcraft','sound':'soundcraft'}
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def inside(root,path):return root.absolute() in path.resolve().parents and not path.is_symlink() and root.resolve()==root.absolute()
def run(cmd,cwd,env,log):
 print('RUN',cmd,flush=True)
 with log.open('ab') as f:
  result=subprocess.run([str(x) for x in cmd],cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT)
 if result.returncode:raise RuntimeError(f'Build command failed ({result.returncode}); see {log}')
def copy_support(old,stage):
 if not old.exists():return
 for p in old.rglob('*'):
  rel=p.relative_to(old)
  if any(x.startswith('.') for x in rel.parts) or not p.is_file():continue
  if p.suffix=='.wasm' or p.name.startswith(('ACA_','academy-web-sha256')):continue
  if len(rel.parts)==1 and re.match(r'(?:photo|vector|sound)craft[-_]web.*\.js$',p.name):continue
  to=stage/rel;to.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,to)
class Tag(HTMLParser):
 def handle_starttag(self,name,attrs):self.attrs=dict(attrs)
def build(slug):
 repo=ROWS[slug];source=BASE/repo;web=source/f'apps/{repo}-web';stage=source/'dist/web-aca090';out=source/'dist/web'
 if not inside(source/'dist',stage):raise RuntimeError('Unsafe staging directory')
 if stage.exists():
  if not(stage/'ACA_STAGE_OWNED.json').exists():raise RuntimeError('Refusing unowned native stage')
  shutil.rmtree(stage)
 stage.mkdir(parents=True);(stage/'ACA_STAGE_OWNED.json').write_text('{"tool":"aca-native-090"}',encoding='utf-8')
 copy_support(out,stage)
 env=os.environ.copy();env.update({'CARGO_HOME':str(TOOL/'cargo'),'RUSTUP_HOME':str(TOOL/'rustup'),'RUSTUP_TOOLCHAIN':'1.95.0','CARGO_TARGET_DIR':str(TARGET),'CARGO_BUILD_JOBS':'1','CARGO_NET_OFFLINE':'true','BINARYEN_CORES':'1'})
 env['PATH']=str(TOOL/'cargo/bin')+os.pathsep+str(BINDGEN.parent)+os.pathsep+env.get('PATH','')
 for key in ['CARGO_PROFILE_RELEASE_LTO','CARGO_PROFILE_RELEASE_CODEGEN_UNITS']:env.pop(key,None)
 args=[CARGO,'rustc','--release','--locked','--offline','--target','wasm32-unknown-unknown','-p',repo+'-web']
 lib=repo in ['effectcraft','filmcraft'];args+=['--lib'] if lib else ['--bin',repo+'-web']
 if repo=='photocraft':args+=['--features','heif']
 args+=['--','-C','lto=thin','-C','codegen-units=16']
 baseline=max((p.stat().st_size for p in out.glob('*.wasm')),default=0)
 log=source/'aca-090-final-build.log';started=time.monotonic();run(args,source,env,log)
 stem=repo+'_web' if lib else repo+'-web';raw=TARGET/'wasm32-unknown-unknown/release'/f'{stem}.wasm'
 run([BINDGEN,'--target','web','--no-typescript','--weak-refs','--out-dir',stage,raw],source,env,log)
 wasm=stage/(stem.replace('-','_')+'_bg.wasm');glue=stage/(stem.replace('-','_')+'.js')
 # wasm-bindgen preserves hyphens in the output stem for binaries.
 if not wasm.exists():wasm=stage/(stem+'_bg.wasm');glue=stage/(stem+'.js')
 if not wasm.exists() or not glue.exists():raise RuntimeError('Missing generated wasm-bindgen output')
 if OPT.exists():
  opt=stage/'optimized.wasm';run([OPT,'-Oz','--converge','--enable-bulk-memory','--enable-nontrapping-float-to-int','--enable-sign-ext','--enable-mutable-globals','--enable-reference-types','--enable-multivalue','--enable-simd',wasm,'-o',opt],source,env,log);opt.replace(wasm)
 budget=max(64*1024*1024,baseline)
 if wasm.stat().st_size>budget:raise RuntimeError(f'Profile candidate exceeds comparison budget {budget}; use pinned profile or inspect payload tradeoff')
 entry=web/('web/index.html' if lib else 'index.html');html=entry.read_text(encoding='utf-8')
 if lib:
  for p in entry.parent.iterdir():
   if p.is_file() and not p.name.endswith('.attribution'):shutil.copy2(p,stage/p.name)
  html=html.replace('__FC_BUILD__',sha(wasm)[:16])
 else:
  def link(m):
   t=Tag();t.feed(m.group(0));a=t.attrs
   if a.get('rel')=='copy-file':
    src=(entry.parent/a['href']).resolve();folder=stage/a.get('data-target-path','');folder.mkdir(parents=True,exist_ok=True);shutil.copy2(src,folder/src.name)
   return ''
  html=re.sub(r'<link\b[^>]*\bdata-trunk\b[^>]*>',link,html,flags=re.S)
  # Use hashes for every compiled binary/glue URL, matching Trunk's cache semantics.
  wasm_new=wasm.with_name(stem+'-'+sha(wasm)[:16]+'_bg.wasm');wasm.rename(wasm_new);wasm=wasm_new
  glue_new=glue.with_name(stem+'-'+sha(glue)[:16]+'.js');glue.rename(glue_new);glue=glue_new
  module=f'''<script type="module">import init,* as bindings from './{glue.name}';const wasm=await init({{module_or_path:'./{wasm.name}'}});window.wasmBindings=bindings;dispatchEvent(new CustomEvent('TrunkApplicationStarted',{{detail:{{wasm}}}}));</script>'''
  html=html.replace('</head>',module+'\n</head>')
 (stage/'index.html').write_text(html,encoding='utf-8')
 for name in ['LICENSE-MIT','LICENSE-APACHE','NOTICE','ATTRIBUTION.md']:
  if(source/name).exists():shutil.copy2(source/name,stage/name)
 # New assets are source-owned and always copied explicitly.
 shutil.copy2(entry.parent/'academy-bridge.js',stage/'academy-bridge.js')
 if slug=='motion':(stage/'effectcraft-config.json').write_text('{"startup":"empty"}\n',encoding='utf-8')
 if slug=='motion':
  files=['./']+['./'+f.relative_to(stage).as_posix() for f in sorted(stage.rglob('*')) if f.is_file() and f.name!='sw.js' and not f.name.startswith('ACA_')]
  fingerprint='\n'.join(f'{f}:{sha(stage/f[2:])}' for f in files if f!='./')
  version=hashlib.sha256(fingerprint.encode()).hexdigest()[:16]
  worker=(entry.parent/'sw.js').read_text(encoding='utf-8').replace('__EC_VERSION__',version).replace('__EC_FILES__',json.dumps(files))
  (stage/'sw.js').write_text(worker,encoding='utf-8')
 profile={'tool':'aca-native-090','profile':'release dependencies unchanged; final web crate -C lto=thin -C codegen-units=16','uiPackageCodegenUnits':1 if slug=='photo' else 16,'jobs':1,'rust':'1.95.0','wasmBindgen':'0.2.129','wasmOpt':'version123 -Oz --converge' if OPT.exists() else 'not available','baselineWasmBytes':baseline,'comparisonBudgetBytes':budget,'elapsedSeconds':round(time.monotonic()-started,2),'wasm':{'path':wasm.name,'bytes':wasm.stat().st_size,'sha256':sha(wasm)}}
 (stage/'ACA_BUILD_PROFILE.json').write_text(json.dumps(profile,indent=2)+'\n',encoding='utf-8')
 backup=source/'dist/web-pre-final090'
 if out.exists():
  if not inside(source/'dist',out):raise RuntimeError('Unsafe native current-build directory')
  if backup.exists():
   if not(backup/'ACA_BACKUP_OWNED.json').exists():raise RuntimeError('Refusing unowned native backup')
   if not inside(source/'dist',backup):raise RuntimeError('Unsafe native previous-build backup')
   shutil.rmtree(backup)
  out.rename(backup)
  (backup/'ACA_BACKUP_OWNED.json').write_text('{"tool":"aca-native-090"}',encoding='utf-8')
 stage.rename(out)
 print(json.dumps({'app':slug,**profile}),flush=True)
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--only',choices=list(ROWS));a=p.parse_args()
 for slug in ([a.only] if a.only else ROWS):build(slug)
