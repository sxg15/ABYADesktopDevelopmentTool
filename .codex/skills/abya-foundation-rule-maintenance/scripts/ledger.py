"""Persistent rule/document matrix. No network calls or gameplay-document writes."""
from pathlib import Path
from contextlib import contextmanager
from datetime import datetime, timezone
from html import escape
import argparse, copy, json, os, re, uuid

ROOT=Path(__file__).resolve().parents[1]
DEFAULT=ROOT/'state/ledger.json'
STATUSES={'pending':'待处理','needs_review':'待核对','in_progress':'进行中','partial':'部分完成','applied':'已应用','blocked':'受阻','not_applicable':'不适用'}

def read(path): return json.loads(Path(path).read_text(encoding='utf-8-sig'))
def stamp(): return datetime.now(timezone.utc).isoformat(timespec='seconds')
def key(a): return (a['document_id'],a['rule_id'],a['rule_version'])
def applies(rule,doc): return rule.get('scope','all')=='all' or doc['document_id'] in rule.get('document_ids',[])

def proof_valid(e):
    return all(e.get(k) for k in ('document_url','revision_id','checked_at','content_excerpt','package_check','verification_level','rule_id','rule_version'))

def validate(data):
    assert data['schema_version']==1
    rules={r['id']:r for r in data['rules']};docs={d['document_id']:d for d in data['documents']}
    assert len(rules)==len(data['rules']) and len(docs)==len(data['documents'])
    seen=set()
    for r in rules.values():
        assert r['version']>=1 and r.get('source') and r.get('requirements') and r.get('new_document_policy')=='design_now'
    for a in data['applications']:
        assert key(a) not in seen;seen.add(key(a))
        assert a['document_id'] in docs and a['rule_id'] in rules and a['rule_version']<=rules[a['rule_id']]['version']
        assert a['status'] in STATUSES
        if a['status']=='applied':
            e=a.get('evidence',{})
            assert proof_valid(e), 'Applied requires readback/package evidence'
            assert e['rule_id']==a['rule_id'] and e['rule_version']==a['rule_version'],'Evidence belongs to another rule version'
            assert find_doc(data,e['document_url'])['document_id']==a['document_id'],'Evidence belongs to another document'
        if a['status']=='not_applicable': assert a.get('note'), 'Not applicable requires a reason'
    for d in docs.values():
        for r in rules.values():
            if r.get('active',True) and applies(r,d):
                assert (d['document_id'],r['id'],r['version']) in seen,'Missing current rule/document record'
    return {'rules':len(rules),'documents':len(docs),'applications':len(seen)}

def fill(data,existing_documents_are_legacy=False):
    seen={key(a) for a in data['applications']}
    for d in data['documents']:
        for r in data['rules']:
            k=(d['document_id'],r['id'],r['version'])
            if r.get('active',True) and applies(r,d) and k not in seen:
                legacy=existing_documents_are_legacy or d.get('cohort','legacy')=='legacy'
                data['applications'].append(dict(document_id=d['document_id'],rule_id=r['id'],rule_version=r['version'],adoption_mode='legacy_migration' if legacy else 'design_now',status='pending' if legacy else 'needs_review',note='等待用户发起旧稿迁移' if legacy else '新稿直接遵循规则，交付时需核验证据',runtime_validation='not_tested'))

@contextmanager
def transaction(path):
    path.parent.mkdir(parents=True,exist_ok=True);lock=path.with_suffix('.lock')
    fd=os.open(lock,os.O_CREAT|os.O_EXCL|os.O_WRONLY)
    try:
        os.close(fd);data=read(path)
        yield data
        validate(data)
        temp=path.with_name(path.name+'.'+uuid.uuid4().hex+'.tmp')
        try:
            temp.write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');os.replace(temp,path)
        finally:
            if temp.exists():temp.unlink()
    finally: lock.unlink()

def find_doc(data,query):
    token=lambda s:str(s).rstrip('/').split('/')[-1].split('?')[0].split('#')[0]
    match=[d for d in data['documents'] if query in (d['document_id'],d['title'],d.get('game'),d['url'],*d.get('aliases',[])) or token(query) in {token(v) for v in (d['document_id'],d['url'],*d.get('aliases',[]))}]
    if not match:match=[d for d in data['documents'] if query in d['title'] or query in d.get('game','')]
    if len(match)!=1:raise ValueError('Document must resolve uniquely; found '+str(len(match)))
    return match[0]

def query(data,document=None,rule=None):
    doc=find_doc(data,document) if document else None
    latest={r['id']:r['version'] for r in data['rules'] if r.get('active',True)}
    rows=[]
    for a in data['applications']:
        if a['rule_version']!=latest.get(a['rule_id']):continue
        if doc and a['document_id']!=doc['document_id']:continue
        if rule and a['rule_id']!=rule:continue
        d=next(d for d in data['documents'] if d['document_id']==a['document_id'])
        cohort='legacy' if a.get('adoption_mode','legacy_migration' if d.get('cohort','legacy')=='legacy' else 'design_now')=='legacy_migration' else 'new'
        rows.append(dict(a,game=d['game'],document_url=d['url'],ledger_url=d.get('ledger_url',data.get('cloud',{}).get('url')),cohort=cohort))
    return {'cloud':data.get('cloud',{}),'document':doc,'rules':[r for r in data['rules'] if r.get('active',True) and (not rule or r['id']==rule)],'items':rows,'coverage':data.get('inventory',{})}

def render(data):
    def p(t):return '<p>'+escape(str(t))+'</p>'
    def h(t,n=1):return f'<h{n} seq="auto">'+escape(t)+f'</h{n}>'
    def a(label,url):return '<a href="'+escape(url,quote=True)+'">'+escape(label)+'</a>'
    def table(head,rows):
        return '<table><thead><tr>'+''.join('<th background-color="light-gray">'+p(x)+'</th>' for x in head)+'</tr></thead><tbody>'+''.join('<tr>'+''.join('<td><p>'+x+'</p></td>' for x in row)+'</tr>' for row in rows)+'</tbody></table>'
    active=[r for r in data['rules'] if r.get('active',True)]
    current=query(data)['items'];counts={s:sum(a['status']==s for a in current) for s in STATUSES}
    result=['<title>ABYA｜底层规则维护台账</title>',p('登记日期：'+data['business_date']+'。本台账记录旧玩法文档的规则应用与待维护事项；登记或查询不自动修改玩法正文，仅在用户发起后执行指定旧稿迁移。新文档在设计时直接遵守所有生效规则。'),p(f'已登记{len(data["documents"])}份玩法文档、{len(active)}条生效规则、{len(current)}条当前版本应用记录。已应用{counts["applied"]}条；待处理{counts["pending"]}条；待核对{counts["needs_review"]}条；其余进行中/部分完成/受阻见各文档分节。'),h('查询与执行边界'),p('在本页搜索玩法名或原文档链接即可查看对应事项；也可在对话中说“用底层规则批量维护查询滑冰待维护事项”或“规则BR-20261111-001哪些还没应用”。登记/查询只更新台账；说“将规则X应用到文档Y”时才执行指定旧稿迁移。'),p('状态表示该版本规则在文档及交接包的落实情况，和真人体验验证分开。待处理不代表一定完全不符合；已有相关说明但缺完整核验证据时标待核对。'),h('生效规则队列')]
    for r in active:
        result += [h(r['id']+'｜'+r['title'],2),p('版本v'+str(r['version'])+'；确认日期'+r['confirmed_date']+'；适用范围：'+r['scope_label']+'。新文档立即纳入设计，旧稿按本台账延期迁移。')]
        result += [p(t) for t in r['requirements']]
        result += [p('应用核验：'+t) for t in r['acceptance']]
    result += [h('文档状态总览')]
    headers=['玩法与原文档','普通20秒得分标准','全流程键盘与白圈提示']
    matrix=[]
    for d in data['documents']:
        vals=[a(d['game'],d['url'])]
        for r in active:
            row=next(x for x in current if x['document_id']==d['document_id'] and x['rule_id']==r['id'])
            vals.append(escape(STATUSES[row['status']]))
        matrix.append(vals)
    # The first two current rules have fixed table headings; future rules use their titles.
    if len(active)!=2:headers=['玩法与原文档']+[r['title'] for r in active]
    result.append(table(headers,matrix));result.append(h('按文档查询'))
    for d in data['documents']:
        result += [h(d['game']+'｜待维护事项',2),'<p>'+a(d['title'],d['url'])+'</p>',p('基线：'+d.get('baseline','尚未核对')+'。新稿直接适用与旧稿迁移按各规则记录分别判断。')]
        rows=[]
        for r in active:
            x=next(x for x in current if x['document_id']==d['document_id'] and x['rule_id']==r['id'])
            rows.append([escape(r['id']+' v'+str(r['version'])),escape(STATUSES[x['status']]),escape(x.get('plan',r.get('migration_plan','按规则检查并迁移'))),escape(x.get('note','')),escape(x.get('evidence',{}).get('content_excerpt','尚无已应用核验证据'))])
        result.append(table(['规则','状态','执行内容与后续验证','备注','已应用依据'],rows))
    result += [h('状态判定与目录覆盖'),p('pending待处理；needs_review待核对；in_progress进行中；partial部分完成；applied已应用；blocked受阻；not_applicable不适用。应用完成需要正文、必要配图/输入包与远端回读证据；仅Skill更新或本地草稿不算。规则升级会新增版本记录，旧版完成记录保留。'),p(data['inventory']['coverage_note']),'<p>'+a('维护表来源',data['inventory']['source_url'])+'</p>',p('人工备注区：可在此补充遗漏文档、优先级或实际试玩数据；后续更新须先读取并保留这些备注。')]
    return ''.join(result)

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--state',type=Path,default=DEFAULT)
    sp=ap.add_subparsers(dest='command',required=True)
    sp.add_parser('validate');sp.add_parser('rules')
    q=sp.add_parser('query');q.add_argument('--document');q.add_argument('--rule')
    for name in ('register-document','upsert-rule'):
        q=sp.add_parser(name);q.add_argument('--input',type=Path,required=True)
    q=sp.add_parser('set-status');q.add_argument('--document',required=True);q.add_argument('--rule',required=True);q.add_argument('--status',choices=STATUSES,required=True);q.add_argument('--evidence',type=Path);q.add_argument('--note')
    q=sp.add_parser('render');q.add_argument('--output',type=Path,required=True)
    args=ap.parse_args()
    if args.command in ('register-document','upsert-rule','set-status'):
        with transaction(args.state) as data:
            if args.command=='register-document':
                incoming=read(args.input);existing=next((d for d in data['documents'] if d['document_id']==incoming['document_id']),None)
                if existing:existing.update(incoming)
                else:data['documents'].append(incoming)
                fill(data)
            elif args.command=='upsert-rule':
                incoming=read(args.input);old=next((r for r in data['rules'] if r['id']==incoming['id']),None)
                if old:
                    assert incoming['version']>old['version'],'Changed rules require a higher version'
                    data.setdefault('rule_history',[]).append(copy.deepcopy(old));data['rules'][data['rules'].index(old)]=incoming
                else:data['rules'].append(incoming)
                fill(data,existing_documents_are_legacy=True)
            else:
                d=find_doc(data,args.document);r=next(r for r in data['rules'] if r['id']==args.rule)
                row=next(a for a in data['applications'] if key(a)==(d['document_id'],r['id'],r['version']))
                evidence=read(args.evidence) if args.evidence else {}
                if args.status=='applied':assert proof_valid(evidence),'Missing readback/package evidence'
                if args.status in ('blocked','partial','not_applicable'):assert args.note,'This status requires a specific reason or unfinished items'
                previous=copy.deepcopy(row);row['status']=args.status
                if args.note:row['note']=args.note
                if evidence:row['evidence']=evidence
                data.setdefault('history',[]).append({'at':stamp(),'before':previous,'after':copy.deepcopy(row)})
            data.setdefault('history',[]).append({'at':stamp(),'operation':args.command})
        print(json.dumps(validate(read(args.state)),ensure_ascii=False));return
    data=read(args.state)
    if args.command=='validate':output=validate(data)
    elif args.command=='rules':output=[r for r in data['rules'] if r.get('active',True)]
    elif args.command=='query':output=query(data,args.document,args.rule)
    elif args.command=='render':
        args.output.write_text(render(data),encoding='utf-8');output={'output':str(args.output),'characters':len(render(data))}
    print(json.dumps(output,ensure_ascii=False,indent=2))

if __name__=='__main__':main()
