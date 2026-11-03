"""Serial native gate runner; runtime/browser QA remains a separate root gate."""
from pathlib import Path
import json, subprocess, sys, time
from aca_build_native import BASE, ROWS, build
ROOT=BASE.parent/'alejandro-creative-academy-paladin'
STATUS=BASE/'photocraft/aca-native-090-status.json'
def validate(slug):
 root=BASE/ROWS[slug]/'dist/web';profile=json.loads((root/'ACA_BUILD_PROFILE.json').read_text(encoding='utf-8'));wasm=root/profile['wasm']['path'];data=wasm.read_bytes()
 if data[:8]!=b'\x00asm\x01\x00\x00\x00':raise ValueError('Invalid WASM envelope')
 def u32(pos):
  value=0
  for shift in range(0,35,7):
   if pos>=len(data):raise ValueError('Truncated WASM')
   b=data[pos];pos+=1;value|=(b&127)<<shift
   if not b&128:return value,pos
  raise ValueError('Invalid WASM length')
 pos=8;exports=set()
 while pos<len(data):
  section=data[pos];size,pos=u32(pos+1);end=pos+size
  if end>len(data):raise ValueError('Truncated WASM section')
  if section==7:
   count,pos=u32(pos)
   for _ in range(count):
    n,pos=u32(pos);name=data[pos:pos+n].decode();pos+=n
    if pos>=end:raise ValueError('Invalid WASM export')
    pos+=1;_,pos=u32(pos);exports.add(name)
  pos=end
 for name in ['academyTheme','academyReady','academyThemeState']:
  if name not in exports:raise ValueError('Missing native export '+name)
 glue='\n'.join(p.read_text(encoding='utf-8') for p in root.glob('*.js'))
 if any(('function '+n) not in glue for n in ['academyTheme','academyReady','academyThemeState']):raise ValueError('Missing JS native appearance API')
 html=(root/'index.html').read_text(encoding='utf-8')
 if 'academy-bridge.js' not in html:raise ValueError('Missing bridge bootstrap')
 if '__EC_VERSION__' in (root/'sw.js').read_text(encoding='utf-8') if (root/'sw.js').exists() else False:raise ValueError('Unversioned worker')
 return {'buildVerified':True,'runtimeTested':False,'wasm':profile['wasm'],'profile':profile['profile']}
def record(state):STATUS.write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(json.dumps(state),flush=True)
state={'phase':'waiting-photo','apps':{}};record(state);slug='photo'
try:
 deadline=time.monotonic()+3600
 while not(BASE/'photocraft/dist/web/ACA_BUILD_PROFILE.json').exists():
  if time.monotonic()>deadline:raise TimeoutError('Photo final build did not publish a profile in one hour')
  time.sleep(5)
 for slug in ['photo','vector','motion','film','sound']:
  state['phase']='building-'+slug;record(state)
  if slug!='photo':build(slug)
  state['apps'][slug]=validate(slug)
  subprocess.run([sys.executable,str(ROOT/'tools/sync-academy-apps.py'),'--only',slug],cwd=ROOT,check=True)
  state['phase']='synced-'+slug;record(state)
 state['phase']='all-builds-synced';record(state)
except Exception as error:
 state.update(phase='failed',failedApp=slug,error=str(error));record(state);raise
