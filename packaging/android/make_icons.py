#!/usr/bin/env python3
"""Renders the launcher icons from Horizon's brand mark (docs/media/brand/icon.svg, 16x16 grid)."""
import os
from PIL import Image, ImageDraw
BG=(0x16,0x18,0x1f,255); FG=(0xec,0xed,0xf3,255); DIM=(0xec,0xed,0xf3,92); AC=(0x9a,0x86,0xff,255)
RECTS=[((4,3,6,7),FG),((10,3,12,7),FG),((4,8,6,12),DIM),((10,8,12,12),DIM),((0,7,16,8),AC)]
here=os.path.dirname(os.path.abspath(__file__)); res=os.path.join(here,"res")
def mark(size, scale, off, bg=True, round_=False, ss=8):
    S=size*ss; im=Image.new("RGBA",(S,S),(0,0,0,0)); d=ImageDraw.Draw(im,"RGBA")
    u=S*scale/16; o=S*off
    for (x0,y0,x1,y1),c in RECTS:
        if not bg and x1-x0==16: x0,x1=-16,32  # accent line spans full layer
        layer=Image.new("RGBA",(S,S),(0,0,0,0)); ImageDraw.Draw(layer).rectangle([o+x0*u,o+y0*u,o+x1*u-1,o+y1*u-1],fill=c)
        im.alpha_composite(layer)
    if bg:
        base=Image.new("RGBA",(S,S),(0,0,0,0)); m=Image.new("L",(S,S),0)
        md=ImageDraw.Draw(m)
        if round_: md.ellipse([o,o,S-o-1,S-o-1],fill=255)
        else: md.rounded_rectangle([o,o,S-o-1,S-o-1],radius=3.5*u,fill=255)
        base.paste(Image.new("RGBA",(S,S),BG),(0,0),m); base.alpha_composite(im)
        clip=Image.new("RGBA",(S,S),(0,0,0,0)); clip.paste(base,(0,0),m); im=clip
    return im.resize((size,size),Image.LANCZOS)
for name,px in [("mdpi",48),("hdpi",72),("xhdpi",96),("xxhdpi",144),("xxxhdpi",192)]:
    d=os.path.join(res,f"mipmap-{name}"); os.makedirs(d,exist_ok=True)
    mark(px,15/16,1/32).save(f"{d}/ic_launcher.png")
    mark(px,15/16,1/32,round_=True).save(f"{d}/ic_launcher_round.png")
    fp=px*108//48  # adaptive foreground: 108dp canvas, mark in 66dp safe zone
    mark(fp,60/108*16/16,24/108,bg=False).save(f"{d}/ic_launcher_foreground.png")
