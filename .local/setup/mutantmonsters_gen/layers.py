import subprocess, sys, re, json
def javap(path):
    return subprocess.run(['javap','-c','-p','-constants',path],capture_output=True,text=True).stdout
def methods(txt):
    out={}; cur=None
    for line in txt.splitlines():
        m=re.match(r'  (?:public |private |protected )?(?:static )?(?:final )?(\S+) (\w+)\((.*)\);',line)
        if m: cur=m.group(2)+'('+m.group(3)+')'; out[cur]={'ret':m.group(1),'code':[]}; continue
        m=re.match(r'\s+(\d+): (\w+)\s*(.*)',line)
        if m and cur: out[cur]['code'].append((int(m.group(1)),m.group(2),m.group(3)))
    return out
class Part:
    def __init__(s,name): s.name=name; s.cubes=[]; s.pose=None; s.children=[]
    def js(s): return {'name':s.name,'cubes':s.cubes,'pose':s.pose,'children':[c.js() for c in s.children]}
class Builder:
    def __init__(s): s.u=0; s.v=0; s.mirror=False; s.cubes=[]
def run(code, args):
    try: return run1(code,args)
    except Exception as e:
        raise Exception(str(e)+' @ '+repr(LAST[0]))
LAST=[None]
def run1(code, args):
    global LOCAL
    st=[]; loc=dict(enumerate(args)); pcmap={pc:i for i,(pc,_,_) in enumerate(code)}; i=0; steps=0
    while i<len(code):
        steps+=1
        if steps>200000: raise Exception('loop')
        pc,op,arg=code[i]; i+=1; LAST[0]=(pc,op,arg,list(st)[-4:])
        def cval(): 
            m=re.search(r'// (?:float|int|double) (-?[\d.E+-]+|NaN)f?',arg)
            if m: return float(m.group(1))
            m=re.search(r'// String (.*)',arg)
            return m.group(1) if m else None
        if op in('ldc','ldc_w','ldc2_w'): st.append(cval())
        elif op.startswith('fconst_') or op.startswith('iconst_'): v=op.split('_')[1]; st.append(-1.0 if v=='m1' else float(v))
        elif op.startswith('dconst_'): st.append(float(op[-1]))
        elif op in('bipush','sipush'): st.append(float(arg.split()[0]))
        elif re.match(r'[afid]load(_\d)?$',op): n=int(op.split('_')[1]) if '_' in op else int(arg.split()[0]); st.append(loc.get(n))
        elif re.match(r'[afid]store(_\d)?$',op): n=int(op.split('_')[1]) if '_' in op else int(arg.split()[0]); loc[n]=st.pop()
        elif op=='iinc': n,d=arg.split(','); loc[int(n)]+=float(d)
        elif op=='dup': st.append(st[-1])
        elif op=='pop': st.pop()
        elif op in('fneg','ineg','dneg'): st.append(-st.pop())
        elif op in('fmul','imul','dmul'): b=st.pop();a=st.pop();st.append(a*b)
        elif op in('fadd','iadd','dadd'): b=st.pop();a=st.pop();st.append(a+b)
        elif op in('fsub','isub','dsub'): b=st.pop();a=st.pop();st.append(a-b)
        elif op in('fdiv','ddiv'): b=st.pop();a=st.pop();st.append(a/b)
        elif op=='idiv': b=st.pop();a=st.pop();st.append(float(int(a/b)))
        elif op in('i2f','f2i','i2d','d2f','f2d','d2i'): v=st.pop(); st.append(float(int(v)) if op.endswith('2i') else v)
        elif op in('if_icmpge','if_icmplt','if_icmpgt','if_icmple','if_icmpeq','if_icmpne'):
            b=st.pop();a=st.pop(); t=int(arg.split()[0])
            c={'ge':a>=b,'lt':a<b,'gt':a>b,'le':a<=b,'eq':a==b,'ne':a!=b}[op[-2:]]
            if c: i=pcmap[t]
        elif op in('ifeq','ifne','iflt','ifge','ifgt','ifle'):
            a=st.pop(); t=int(arg.split()[0]); c={'eq':a==0,'ne':a!=0,'lt':a<0,'ge':a>=0,'gt':a>0,'le':a<=0}[op[2:]]
            if c: i=pcmap[t]
        elif op=='goto': i=pcmap[int(arg.split()[0])]
        elif op=='fcmpl' or op=='fcmpg': b=st.pop();a=st.pop(); st.append(float((a>b)-(a<b)))
        elif op=='new': st.append(('new',arg.split('class ')[-1]))
        elif op=='invokespecial':
            if 'class_5605."<init>":(F)' in arg: g=st.pop(); st.pop(); st.pop(); st.append(('def',g,g,g))
            elif 'class_5605."<init>":(FFF)' in arg: c=st.pop();b=st.pop();a=st.pop(); st.pop(); st.pop(); st.append(('def',a,b,c))
            elif 'class_5609."<init>"' in arg: st.pop(); st.pop(); st.append(Part('root'))
            else: raise Exception('spec '+arg)
        elif op=='getstatic':
            if 'class_5603.field_27701' in arg: st.append({'offset':[0,0,0],'rot':[0,0,0]})
            elif 'class_5605.field_27715' in arg: st.append(('def',0,0,0))
            else: raise Exception('gs '+arg)
        elif op=='areturn': return st.pop()
        elif op in('invokevirtual','invokestatic'):
            s=arg
            if 'class_5609.method_32111' in s: st.pop(); st.append(None) if False else st.append(ROOT[0])
            elif 'class_5606.method_32108' in s: st.append(Builder())
            elif 'class_5606.method_32101' in s: v=st.pop();u=st.pop();b=st[-1];b.u,b.v=int(u),int(v)
            elif 'class_5606.method_32096' in s: st[-1].mirror=True
            elif 'class_5606.method_32106' in s: m=st.pop(); st[-1].mirror=bool(m)
            elif 'class_5606.method_32097' in s:
                a=[st.pop() for _ in range(6)][::-1]; b=st[-1]; b.cubes.append({'uv':[b.u,b.v],'origin':a[:3],'size':a[3:],'mirror':b.mirror,'grow':[0,0,0]})
            elif 'class_5606.method_32098' in s:
                d=st.pop(); a=[st.pop() for _ in range(6)][::-1]; b=st[-1]; b.cubes.append({'uv':[b.u,b.v],'origin':a[:3],'size':a[3:],'mirror':b.mirror,'grow':list(d[1:])})
            elif 'class_5605.method_32094' in s: g=st.pop(); d=st.pop(); st.append(('def',d[1]+g,d[2]+g,d[3]+g))
            elif 'class_5603.method_32090' in s: z=st.pop();y=st.pop();x=st.pop(); st.append({'offset':[x,y,z],'rot':[0,0,0]})
            elif 'class_5603.method_32091' in s: a=[st.pop() for _ in range(6)][::-1]; st.append({'offset':a[:3],'rot':a[3:]})
            elif 'class_5610.method_32117' in s:
                pose=st.pop(); b=st.pop(); name=st.pop(); par=st.pop(); p=Part(name); p.cubes=b.cubes; p.pose=pose
                par.children=[c for c in par.children if c.name!=name]+[p]; st.append(p)
            elif 'class_5610.method_32116' in s: n=st.pop(); par=st.pop(); st.append([c for c in par.children if c.name==n][0])
            elif 'class_5607.method_32110' in s: h=st.pop();w=st.pop();m=st.pop(); st.append({'w':int(w),'h':int(h),'root':ROOT[0].js()})
            elif 'class_3532' in s or 'java/lang/Math' in s:
                import math; v=st.pop(); st.append(v)
            elif 'String.valueOf' in s or 'StringBuilder' in s or 'makeConcatWithConstants' in s: raise Exception('str '+s)
            else:
                # call into same class static method
                m=re.search(r'Method (?:([\w/$]+)\.)?(\w+):\((.*?)\)',s)
                tgt=None
                CALLEE[0]=None
                if m and m.group(1) and m.group(1).startswith('fuzs/'): tgt=cls(m.group(1)).get(m.group(2)); CALLEE[0]=m.group(1)
                elif m: tgt=LOCAL.get(m.group(2))
                if tgt is not None:
                    m=re.search(r'Method (?:[\w/$]+\.)?(\w+):\((.*?)\)',s)
                    argc=len(re.findall(r'L[^;]+;|[FIZDJ]',m.group(2))); a=[st.pop() for _ in range(argc)][::-1]
                    sv=(LOCAL,BSM[0])
                    if CALLEE[0]: LOCAL=cls(CALLEE[0]); BSM[0]=bsms(CALLEE[0])
                    r=run(tgt,a)
                    LOCAL,BSM[0]=sv
                    if not s.endswith(')V') : st.append(r)
                else: raise Exception('call '+s)
        elif op=='invokedynamic':
            m=re.search(r'InvokeDynamic #(\d+):makeConcatWithConstants:\((.*?)\)',arg)
            if not m: raise Exception('indy '+arg)
            argc=len(re.findall(r'L[^;]+;|[FIZDJ]',m.group(2))); a=[st.pop() for _ in range(argc)][::-1]
            rec=BSM[0][int(m.group(1))]; out=''
            for ch in rec:
                if ch=='\x01': v=a.pop(0); out+=str(int(v)) if isinstance(v,float) else str(v)
                else: out+=ch
            st.append(out)
        elif op=='return': return None
        elif op in('checkcast',): pass
        else: raise Exception('op %s %s'%(op,arg))
    return None

ROOT=[None]; LOCAL={}; BSM=[{}]; CALLEE=[None]; CACHE={}
BASE=sys.argv[1]
def cls(name):
    if name not in CACHE:
        CACHE[name]={k.split('(')[0]:v['code'] for k,v in methods(javap(BASE+'/'+name+'.class')).items()}
    return CACHE[name]
def bsms(name):
    t=subprocess.run(['javap','-v',BASE+'/'+name+'.class'],capture_output=True,text=True).stdout
    out={}; cur=None
    for line in t.split('BootstrapMethods:')[-1].splitlines():
        m=re.match(r'\s+(\d+): #',line)
        if m: cur=int(m.group(1)); continue
        m=re.match(r'\s+#\d+ (.*)',line)
        if m and cur is not None and cur not in out: out[cur]=m.group(1).replace('\\u0001','\x01')
    return out
def call(name,meth,args):
    global LOCAL
    LOCAL=cls(name); BSM[0]=bsms(name); ROOT[0]=Part('root')
    return run(LOCAL[meth],args)
M='fuzs/mutantmonsters/client/model/'
D=lambda g:('def',g,g,g)
jobs={'mutant_zombie':('MutantZombieModel',[]),'mutant_skeleton':('MutantSkeletonModel',[]),
 'mutant_creeper':('MutantCreeperModel',[D(0.0)]),'mutant_creeper_armor':('MutantCreeperModel',[D(2.0)]),
 'mutant_enderman':('MutantEndermanModel',[]),'mutant_snow_golem':('MutantSnowGolemModel',[128.0,64.0]),
 'mutant_snow_golem_head':('MutantSnowGolemModel',[64.0,32.0]),'spider_pig':('SpiderPigModel',[]),
 'creeper_minion':('CreeperMinionModel',[D(0.0)]),'creeper_minion_armor':('CreeperMinionModel',[D(2.0)]),
 'endersoul_fragment':('EndersoulFragmentModel',[]),'mutant_arrow':('MutantArrowModel',[]),
 'mutant_skeleton_part':('MutantSkeletonPartModel',[]),'mutant_crossbow':('MutantCrossbowModel',[])}
res={}
for k,(c,a) in jobs.items():
    try: res[k]=call(M+c,'createBodyLayer',a)
    except Exception as e: res[k]='ERR '+repr(e)
print(json.dumps(res,indent=1))
