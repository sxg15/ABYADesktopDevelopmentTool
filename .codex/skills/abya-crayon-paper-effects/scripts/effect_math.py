"""Engine-neutral sampling oracle. Standard library only; reads bundled JSON.

States contain visual data only. No engine API, input locks, scoring or scheduling.
"""
from pathlib import Path
import argparse
import json
import math

ROOT = Path(__file__).resolve().parents[1]
CONFIG = json.loads((ROOT / 'assets/effects.json').read_text(encoding='utf-8'))

def clamp(x):
    return max(0.0, min(1.0, x))

def smooth(x):
    x = clamp(x)
    return x*x*(3-2*x)

def sample(effect_id, time_s, anchor=None):
    """Sample once at elapsed seconds; coordinates are reference logical pixels.

    The host scales all positions/sizes uniformly, supplies the visible content,
    composites texture alpha, and owns cancellation/de-duplication independently.
    """
    if not math.isfinite(time_s):
        raise ValueError('time_s must be finite')
    e = CONFIG['effects'][effect_id]
    if effect_id in ('crayon-check', 'crayon-cross'):
        p = time_s/e['duration_s']
        pos = list(anchor or e['anchor'])
        scale = e['start_scale'] + (e['peak_scale']-e['start_scale'])*smooth(p/e['grow_until']) if p<e['grow_until'] else e['peak_scale']-(e['peak_scale']-1)*smooth((p-e['grow_until'])/e['settle_span'])
        opacity = smooth(p/e['fade_in_fraction'])*(1-smooth((p-e['fade_out_start'])/(1-e['fade_out_start']))) if 0<=p<1 else 0
        return dict(effect=effect_id,visible=opacity>0,asset=e['asset'],position=pos,visible_width=e['visible_width']*scale,scale=scale,alpha=opacity,trim_to_alpha_bbox=True)
    if effect_id == 'paper-burst':
        origin=anchor or e['anchor']; states=[]
        if 0<=time_s<e['duration_s']:
            for index,b in enumerate(e['particles']):
                age=time_s-b['delay'];life=b['life']
                if not 0<age<life:
                    continue
                move=(1-math.exp(-b['drag']*age))/b['drag']
                x=origin[0]+math.cos(b['theta'])*b['v']*move+e['sway_amplitude']*math.sin(age*e['sway_frequency']+b['phase'])*smooth(age/e['sway_ramp_s'])
                y=origin[1]+math.sin(b['theta'])*b['v']*move+e['gravity_quadratic']*age*age
                opacity=smooth(age/e['fade_in_s'])*(1-smooth((age-life*e['fade_out_start_fraction'])/(life*(1-e['fade_out_start_fraction']))))
                flip=math.cos(b['phase']+age*e['flip_frequency'])
                asset=e['sprites'][str(b['i'])]['front' if flip>=0 else 'back']
                states.append(dict(index=index,asset=asset,x=x,y=y,width=b['size'],horizontal_projection=max(e['min_horizontal_projection'],abs(flip)),mirror_x=flip<0,rotation_degrees=b['spin']*age,alpha=opacity))
        return dict(effect=effect_id,visible=bool(states),particles=states,layer='behind_controls_and_text')
    if effect_id == 'page-turn':
        q=smooth((time_s-e['delay_s'])/e['fold_s'])
        projected=abs(math.cos(math.pi*q))
        return dict(effect=effect_id,face='old' if q<e['face_switch_fraction'] else 'new',progress=q,width_scale=max(e['min_width_ratio'],projected),height_scale=1+e['height_gain_ratio']*math.sin(math.pi*q),brightness=1-e['brightness_reduction']*math.sin(math.pi*q),transition_complete=time_s>=e['delay_s']+e['fold_s'],hold_new_content=True)
    raise KeyError(effect_id)

if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('effect',choices=CONFIG['effects'])
    parser.add_argument('--time-ms',type=float,required=True)
    args=parser.parse_args()
    print(json.dumps(sample(args.effect,args.time_ms/1000),ensure_ascii=False,indent=2))
