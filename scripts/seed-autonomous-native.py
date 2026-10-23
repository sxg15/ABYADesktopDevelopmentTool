# Seed owned synthetic approvals only after the isolated APP has exited.
import ctypes, hashlib, json, sqlite3, sys
from datetime import datetime, timezone
from pathlib import Path
root=Path(sys.argv[1]).resolve()
assert json.loads((root/'art-design-native-fixture.json').read_text())['schema']=='abya-art-design-native/v1'
owner=int((root/'owned-app.pid').read_text(encoding='utf-8-sig').strip())
k=ctypes.WinDLL('kernel32',use_last_error=True);k.OpenProcess.restype=ctypes.c_void_p
handle=k.OpenProcess(0x1000,False,owner)
if handle:
 code=ctypes.c_ulong();k.GetExitCodeProcess(ctypes.c_void_p(handle),ctypes.byref(code));k.CloseHandle(ctypes.c_void_p(handle))
 if code.value==259:raise RuntimeError('Close the owned test APP before seeding.')
state=json.loads((root/'autonomous-native.json').read_text(encoding='utf-8'));task=state['task'];workspace=Path(task['workspacePath']).resolve()
assert workspace.is_relative_to(root)
connection=sqlite3.connect(root/'Local/ABYA Desktop Development Tool/Data/app.db')
assert connection.execute('select count(*) from development_tasks').fetchone()[0]==1
record=json.loads(connection.execute('select body from task_production where task_id=?',(task['id'],)).fetchone()[0])
assert record['revision']==1 and not record['approvals']
record['policy']['artDesignVersion']=0
for stage in record['policy']['stages']:
 if stage['id']=='resources':stage['approval']=None
record['currentStage']='implementation'
for key in ['requirements','resources','plan']:record['stages'][key]='passed'
record['stages']['implementation']='in-progress';record['workbench']['goals']=[];record['workbench']['activeGoalId']=None
now=datetime.now(timezone.utc).isoformat()
for kind in ['requirements','plan']:
 text='Controlled scheduler fixture. Complete two registered text-file work units; retain injected platform issue and dependent work. No gameplay, network or asset generation. No user interaction needed.'
 relative=f'artifacts/native-test-{kind}.md';target=workspace/relative;target.parent.mkdir(parents=True,exist_ok=True);target.write_text(text,encoding='utf-8');sha=hashlib.sha256(text.encode()).hexdigest()
 record['documents'][kind]={'kind':kind,'revision':1,'path':relative,'sha256':sha,'content':text,'submittedAt':now,'gameVersion':None,'artifactBindings':{}}
 record['approvals'].append({'kind':kind,'documentRevision':1,'documentHash':sha,'decision':'accepted','feedback':'Synthetic fixture, not real user acceptance','decidedAt':now})
connection.execute('update task_production set body=? where task_id=?',(json.dumps(record,ensure_ascii=False),task['id']));connection.commit();connection.close()
print('Seeded owned synthetic scheduling fixture.')
