from PIL import Image, ImageDraw
from pathlib import Path

# UI assets: 16 pixels/unit, centered pivots; all dimensions multiples of 16.
import argparse
parser=argparse.ArgumentParser(description='Generate reusable comic arcade UI assets; requires Pillow.')
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
OUT=args.output.resolve()
OUT.mkdir(parents=True,exist_ok=True)
INK='#1C1D24'; PAPER='#F4F4F0'; MINT='#00DFA9'; YELLOW='#FFD027'
def save(im,name): im.save(OUT/(name+'.png'))
def icon(d,kind,x,y,s):
    def p(points): return [(x+a*s,y+b*s) for a,b in points]
    if kind=='clock':
        d.ellipse([x+s*.13,y+s*.13,x+s*.87,y+s*.87],outline=INK,width=max(1,int(s*.06)))
        d.line(p([(.5,.28),(.5,.51),(.68,.63)]),fill=INK,width=max(1,int(s*.06)))
    elif kind=='cup':
        d.polygon(p([(.25,.16),(.75,.16),(.68,.6),(.5,.72),(.32,.6)]),fill=YELLOW,outline=INK,width=max(1,int(s*.05)))
        d.line(p([(.5,.68),(.5,.87),(.25,.87),(.75,.87)]),fill=INK,width=max(1,int(s*.06)))
        d.arc([x+s*.05,y+s*.2,x+s*.4,y+s*.58],70,280,fill=INK,width=max(1,int(s*.05)))
        d.arc([x+s*.6,y+s*.2,x+s*.95,y+s*.58],260,110,fill=INK,width=max(1,int(s*.05)))
for kind in ['clock','cup']:
    im=Image.new('RGBA',(128,128)); icon(ImageDraw.Draw(im),kind,0,0,128);save(im,'icon-'+kind)
for color,name in [(PAPER,'white'),(YELLOW,'yellow'),(MINT,'mint')]:
    for state in ['normal','hover','press','disabled']:
        im=Image.new('RGBA',(512,96));d=ImageDraw.Draw(im)
        off=4 if state=='press' else 0
        d.polygon([(10,8),(511,8),(495,95),(0,95)],fill=INK)
        d.polygon([(12,off),(505,off),(488,87+off),(0,87+off)],fill=INK)
        d.polygon([(16,4+off),(499,4+off),(484,82+off),(6,82+off)],fill='#AFB1B5' if state=='disabled' else MINT if state=='hover' else color)
        save(im,'button-'+name+'-'+state)
def card(d,box,fill=PAPER):
    x,y,w,h=box;d.rectangle((x+8,y+8,x+w+8,y+h+8),fill=INK);d.rectangle((x,y,x+w,y+h),fill=fill,outline=INK,width=4)
im=Image.new('RGBA',(688,416));d=ImageDraw.Draw(im);card(d,(0,0,678,404));save(im,'card-panel')
im=Image.new('RGBA',(16,16),'white');save(im,'solid')
for i,col in enumerate([MINT,'#FFAB55','#B195F6','#C8E760']):
    im=Image.new('RGBA',(96,96),col);d=ImageDraw.Draw(im)
    # Deliberately pixel-aligned original avatar illustrations.
    d.rectangle((20,20,75,78),fill=INK);d.rectangle((27,32,68,70),fill='#F0C49C')
    d.rectangle((20,20,75,36),fill=['#263849','#754529','#453654','#3E5D36'][i]);d.rectangle((12,20,83,27),fill=INK)
    d.rectangle((33,43,39,49),fill=INK);d.rectangle((56,43,62,49),fill=INK)
    d.rectangle((40,59,56,64),fill=INK);d.rectangle((20,76,75,95),fill=INK);d.rectangle((29,79,66,95),fill=col)
    save(im,'avatar-'+str(i+1))

im=Image.new('RGBA',(832,832)); ImageDraw.Draw(im).rectangle((2,2,829,829),outline='white',width=6); save(im,'frame-white')


# Separate decoration assets: no baked-in text or screen layout.
im=Image.new('RGBA',(512,96));d=ImageDraw.Draw(im)
d.polygon([(16,8),(511,8),(480,95),(0,95)],fill=INK)
d.polygon([(12,0),(503,0),(473,86),(0,86)],fill=MINT,outline=INK,width=4)
save(im,'ribbon-mint')
im=Image.new('RGBA',(64,64));d=ImageDraw.Draw(im)
d.rectangle((0,0,63,9),fill=MINT);d.rectangle((0,0,9,63),fill=MINT)
save(im,'corner-mint')
im=Image.new('RGBA',(128,128));d=ImageDraw.Draw(im)
d.rectangle((2,2,125,125),outline=MINT,width=5);save(im,'focus-ring')
im=Image.new('RGBA',(192,192));d=ImageDraw.Draw(im)
for y in range(8,192,16):
    for x in range(8,192,16):d.ellipse((x,y,x+3,y+3),fill=(28,29,36,35))
save(im,'halftone')
print('Created',len(list(OUT.glob('*.png'))),'UI assets')
