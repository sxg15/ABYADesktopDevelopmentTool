"""Read-only portable-package and visual-state invariant checks."""
from pathlib import Path
import argparse
import hashlib
import json
import math
import re
import sys
sys.dont_write_bytecode = True
from effect_math import CONFIG, ROOT, sample

def require(ok, message):
    if not ok:
        raise ValueError(message)

def local(relative):
    p=(ROOT/relative).resolve()
    require(p.is_relative_to(ROOT),'Path leaves skill root: '+relative)
    require(p.is_file(),'Missing file: '+relative)
    return p

def validate(images=False):
    expected={'crayon-check','crayon-cross','paper-burst','page-turn'}
    require(set(CONFIG['effects'])==expected,'Only the four selected effects belong in this library')
    manifest=json.loads(local('assets/manifest.json').read_text(encoding='utf-8'))
    for row in manifest['files']:
        p=local(row['path'])
        require(hashlib.sha256(p.read_bytes()).hexdigest()==row['sha256'],'SHA-256 mismatch: '+row['path'])
        if images and p.suffix=='.png':
            from PIL import Image
            im=Image.open(p)
            require(im.mode=='RGBA','PNG must retain RGBA: '+row['path'])
            require(list(im.size)==row['size'],'PNG size mismatch: '+row['path'])
            require(list(im.getchannel('A').getbbox())==row['alpha_bbox'],'PNG trim mismatch: '+row['path'])
            require(im.getchannel('A').getextrema()[0]==0,'PNG background must be transparent: '+row['path'])
    for doc in ROOT.rglob('*.md'):
        for target in re.findall(r'\]\(([^)]+)\)',doc.read_text(encoding='utf-8')):
            if '://' not in target and not target.startswith('#'):
                p=(doc.parent/target.split('#')[0]).resolve()
                require(p.is_relative_to(ROOT) and p.is_file(),'Broken local reference: '+target)
    for k in expected:
        local(CONFIG['effects'][k]['preview'])
    for k in ('crayon-check','crayon-cross'):
        e=CONFIG['effects'][k];local(e['asset'])
        require(not sample(k,-.1)['visible'] and not sample(k,e['duration_s'])['visible'],'Mark endpoint cleanup')
        require(sample(k,.12)['alpha']>.95,'Mark must have a readable hold')
        require(abs(sample(k,e['duration_s']*e['grow_until'])['scale']-e['peak_scale'])<1e-8,'Mark overshoot drift')
    burst=CONFIG['effects']['paper-burst']
    require(len(burst['particles'])==19,'Reference burst particle count changed')
    for pair in burst['sprites'].values():
        local(pair['front']);local(pair['back'])
    require(len(sample('paper-burst',.3)['particles'])==19,'Do not cut the burst off at 300ms')
    require(not sample('paper-burst',burst['duration_s'])['particles'],'Final particle cleanup')
    require(max(p['delay']+p['life'] for p in burst['particles'])<=burst['duration_s'],'Container cuts off a living particle')
    require(len({round(p['life'],3) for p in burst['particles']})>10,'Lifetimes must be staggered')
    for i,p in enumerate(burst['particles']):
        a=sample('paper-burst',p['delay']+p['life']-.001)['particles']
        near=next((x for x in a if x['index']==i),None)
        require(near is not None and near['alpha']<.001,'Particle should reach near-zero alpha before removal')
    normal=sample('paper-burst',.3)['particles']; shifted=sample('paper-burst',.3,[420,381])['particles']
    require(all(abs(b['x']-a['x']-100)<1e-8 and abs(b['y']-a['y']-100)<1e-8 for a,b in zip(normal,shifted)),'Anchors must translate without changing motion')
    e=CONFIG['effects']['page-turn'];mid=e['delay_s']+e['fold_s']/2
    require(sample('page-turn',mid-.001)['face']=='old' and sample('page-turn',mid+.001)['face']=='new','Single-face switch')
    require(sample('page-turn',mid)['width_scale']<.01,'Face swap must happen at the narrow fold')
    require(sample('page-turn',10)['face']=='new' and sample('page-turn',10)['width_scale']==1,'Do not include demo reset in production')
    print(json.dumps(dict(status='passed',effects=sorted(expected),files=len(manifest['files']),images_checked=images,scope='Portable package and sampling invariants; not engine runtime approval'),ensure_ascii=False,indent=2))

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--images',action='store_true')
    validate(parser.parse_args().images)
