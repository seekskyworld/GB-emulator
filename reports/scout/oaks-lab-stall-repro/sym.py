import sys
syms=[]
for l in open('/Users/skyseek/Desktop/project/open/research-platform/GB-emulator/ROM/pokered-src/pokered.sym'):
    l=l.strip()
    if not l or l[0]==';' or ' ' not in l: continue
    a,n=l.split(' ',1)
    if ':' not in a: continue
    b,ad=a.split(':'); syms.append((int(b,16),int(ad,16),n))
def look(bank,addr):
    if addr < 0x4000: bank = 0
    c=[s for s in syms if s[0]==bank and s[1]<=addr and s[1] < 0x8000]
    c.sort(key=lambda s:s[1]); return c[-1] if c else None
for arg in sys.argv[1:]:
    b,a=arg.split(':'); print(arg, look(int(b,16),int(a,16)))
