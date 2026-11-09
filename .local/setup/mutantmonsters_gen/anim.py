# Symbolic translation of a 1.21.1 entity model's setupAnim into straight-line Rust.
import subprocess, sys, re, json
BASE=sys.argv[1]
def javap(path): return subprocess.run(['javap','-c','-p','-constants',BASE+'/'+path+'.class'],capture_output=True,text=True).stdout
def methods(txt):
    out={}; cur=None
    for line in txt.splitlines():
        mc=re.match(r'  (?:public |private |protected )?([\w.$]+)\((.*)\);',line)
        if mc: cur='<init>('+mc.group(2)+')'; out[cur]={'code':[],'static':False}; continue
        m=re.match(r'  (?:public |private |protected )?(?:static )?(?:final )?(\S+) (\w+)\((.*)\);',line)
        if m: cur=m.group(2)+'('+m.group(3)+')'; out[cur]={'code':[],'static':' static ' in ' '+line+' ' }; continue
        m=re.match(r'\s+(\d+): (\w+)\s*(.*)',line)
        if m and cur: out[cur]['code'].append((int(m.group(1)),m.group(2),m.group(3)))
    return out
CACHE={}
def cls(name):
    if name not in CACHE:
        ms=methods(javap(name)); d={}
        for k,v in ms.items(): d.setdefault(k.split('(')[0],[]).append((k,v))
        CACHE[name]=d
    return CACHE[name]
FIELD={'field_3665':9,'field_3657':0,'field_3656':1,'field_3655':2,'field_3654':3,'field_3675':4,'field_3674':5,'field_37938':6,'field_37939':7,'field_37940':8}
class Sym(str): pass
ANIM=[None]
ARM=[0]
USED_ARM=[False]
# Entity state the models read, as the mod passes it: e[0] deathTime, e[1] a zombie's throwHitTick,
# e[2] unused (isLeftHanded is false), e[3] the enderman's active arm, e[4] its arm scale, e[5..9] its held blocks,
# e[9] a snow golem throwing.
ENT_SYM={'field_6213':'e[0]','throwHitTick':'e[1]','getArmScale':'e[4]','isThrowing':'e[9]'}
OUT=[]; T=[0]; PARTS={}; LOG=set()
def tmp(expr):
    T[0]+=1; n='t%d'%T[0]; OUT.append('let %s: f32 = %s;'%(n,expr)); return Sym(n)
def lit(v):
    if isinstance(v,Sym): return v
    if isinstance(v,bool): return '1.0' if v else '0.0'
    if isinstance(v,(int,float)): s=repr(float(v)); return s if not s.startswith('-') else '('+s+')'
    raise Exception('lit %r'%(v,))
def binop(a,b,op):
    if not isinstance(a,Sym) and not isinstance(b,Sym):
        return {'+':a+b,'-':a-b,'*':a*b,'/':(a/b if b else 0.0),'%':(a% b if b else 0.0)}[op]
    return tmp('%s %s %s'%(lit(a),op,lit(b)))
BSMC={}
def bsms(name):
    if name in BSMC: return BSMC[name]
    t=subprocess.run(['javap','-v',BASE+'/'+name+'.class'],capture_output=True,text=True).stdout
    out={}; cur=None
    for line in t.split('BootstrapMethods:')[-1].splitlines():
        m=re.match(r'\s+(\d+): #',line)
        if m: cur=int(m.group(1)); continue
        m=re.match(r'\s+#\d+ (.*)',line)
        if m and cur is not None and cur not in out: out[cur]=m.group(1).replace('\\u0001','\x01')
    BSMC[name]=out; return out
def vadd(a,b):
    return binop(a,b,'+')
def animator(f,name,a):
    """Animator, natively: keyframe phases over the animation tick."""
    if name=='<init>' or name=='update':
        f.update(temp=0,prev=0,correct=False,tm={},ptm={},pt=(a[1] if name=='update' else Sym('pt')))
        return '__none__'
    if name=='setAnimation':
        f['correct']=(ANIM[0]==a[0]); return 1.0 if f['correct'] else 0.0
    if name in('startPhase','setStationaryPhase','resetPhase'):
        if f['correct']: f['prev']=f['temp']; f['temp']=f['temp']+int(a[0])
        if name=='setStationaryPhase': animator_end(f,True)
        if name=='resetPhase': animator_end(f,False)
        return '__none__'
    if name=='endPhase': animator_end(f,False); return '__none__'
    if name in('rotate','move'):
        if f['correct']:
            t=f['tm'].setdefault(a[0],{'r':[0.0,0.0,0.0],'o':[0.0,0.0,0.0]})
            k='r' if name=='rotate' else 'o'
            for j in range(3): t[k][j]=vadd(t[k][j],a[1+j])
        return '__none__'
    if name=='getEntity': return ('ent',)
    raise Exception('Animator.'+name)
def animator_end(f,stationary):
    global OUT
    if not f['correct']: return
    prev,temp=f['prev'],f['temp']
    saved=OUT; OUT=[]
    def apply(m,scale):
        for part,t in m.items():
            i=part_idx(part[1])
            for j in range(3):
                for k,base in (('r',3),('o',0)):
                    v=t[k][j]
                    if isinstance(v,float) and v==0.0: continue
                    OUT.append('p[%d][%d] += %s * %s;'%(i,base+j,lit(v),scale))
    if stationary:
        apply(f['ptm'],'1.0')
    else:
        OUT.append('let f: f32 = ((tick - %s) + %s) / %s;'%(lit(float(prev)),lit(f['pt']),lit(float(temp-prev))))
        OUT.append('let s: f32 = (f * 3.1415927 / 2.0).sin();')
        OUT.append('let inv: f32 = 1.0 - s;')
        apply(f['ptm'],'inv'); apply(f['tm'],'s')
    body=OUT; OUT=saved
    if body:
        OUT.append('if tick >= %s && tick < %s {'%(lit(float(prev)),lit(float(temp))))
        OUT.extend('    '+x for x in body); OUT.append('}')
    if not stationary:
        f['ptm']=f['tm']; f['tm']={}
def jdesc(sig):
    ps=[p.strip() for p in sig.split('(',1)[1].rstrip(')').split(',') if p.strip()]
    prim={'float':'F','int':'I','boolean':'Z','double':'D','long':'J','byte':'B','short':'S','char':'C'}
    out=''
    for p in ps:
        arr=p.count('[]')+(1 if p.endswith('...') else 0); p=p.replace('[]','').replace('...','')
        out+='['*arr+(prim.get(p) or 'L'+p.replace('.','/')+';')
    return out
def part_idx(path):
    if path not in PARTS: PARTS[path]=len(PARTS)
    return PARTS[path]
ENT_DEFAULTS={}
import copy
FORKS=[0]
IPDOM={}
def ipdom(code):
    key=id(code)
    if key in IPDOM: return IPDOM[key]
    n=len(code); pcmap={pc:i for i,(pc,_,_) in enumerate(code)}
    succ=[]
    for i,(pc,op,arg) in enumerate(code):
        if op=='goto': succ.append([pcmap[int(arg.split()[0])]])
        elif op.startswith('if'): succ.append([i+1,pcmap[int(arg.split()[0])]])
        elif op in('return','areturn','freturn','ireturn','dreturn','lreturn','athrow'): succ.append([n])
        elif op in('tableswitch','lookupswitch'): succ.append(None)
        else: succ.append([i+1])
    full=(1<<(n+1))-1
    pd=[full]*n+[1<<n]
    changed=True
    while changed:
        changed=False
        for i in range(n-1,-1,-1):
            ss=succ[i]
            if ss is None: v=(1<<i)|(1<<n)
            else:
                v=full
                for t in ss: v&=pd[t]
                v|=1<<i
            if v!=pd[i]: pd[i]=v; changed=True
    res=[]
    for i in range(n):
        cands=[j for j in range(n) if j!=i and pd[i]>>j&1]
        best=None
        for j in cands:
            if all((pd[j]>>k&1) or k==j for k in cands if k!=j) or best is None:
                pass
        # the closest: the strict postdominator postdominated by all the others
        best=None
        for j in cands:
            if all(pd[j]>>k&1 for k in cands): best=j; break
        res.append(best)
    IPDOM[key]=res
    return res
def zero_like(v):
    if isinstance(v,(float,int,Sym)): return 0.0
    if isinstance(v,dict): return {k:zero_like(x) for k,x in v.items()}
    if isinstance(v,list): return [zero_like(x) for x in v]
    return v
class MergeError(Exception): pass
def merge_vals(x0,x1,a0,a1,decl,memo):
    num=lambda v: isinstance(v,(float,int,Sym)) and not isinstance(v,bool)
    if num(x0) and num(x1):
        if x0==x1 and type(x0)==type(x1): return x0
        T[0]+=1; m='m%d'%T[0]; decl.append('let mut %s: f32 = 0.0;'%m)
        a0.append('%s = %s;'%(m,lit(x0))); a1.append('%s = %s;'%(m,lit(x1))); return Sym(m)
    if isinstance(x0,tuple) and isinstance(x1,tuple) and x0 and x0[0]=='obj' and x1 and x1[0]=='obj':
        if id(x0[2]) in memo: return x0
        memo.add(id(x0[2]))
        keys=set(x0[2])|set(x1[2])
        for k in keys:
            v0=x0[2].get(k, zero_like(x1[2].get(k))); v1=x1[2].get(k, zero_like(v0))
            x0[2][k]=merge_vals(v0,v1,a0,a1,decl,memo)
        return x0
    if isinstance(x0,dict) and isinstance(x1,dict):
        out={}
        for k in list(x0)+[k for k in x1 if k not in x0]:
            v0=x0.get(k, zero_like(x1.get(k))); v1=x1.get(k, zero_like(v0))
            out[k]=merge_vals(v0,v1,a0,a1,decl,memo)
        x0.clear(); x0.update(out); return x0
    if isinstance(x0,list) and isinstance(x1,list) and len(x0)==len(x1):
        for j in range(len(x0)): x0[j]=merge_vals(x0[j],x1[j],a0,a1,decl,memo)
        return x0
    if x0==x1: return x0
    raise MergeError('cannot merge %r / %r'%(str(x0)[:80],str(x1)[:80]))
def fork(owner,code,cond,ti,fi,st,loc,this,stop=None):
    global OUT
    FORKS[0]+=1
    if FORKS[0]>20000: raise Exception('too many forks')
    m=ipdom(code)[fi-1]
    targets=[]
    if m is not None: targets.append(m)
    if stop is not None and stop!=m: targets.append(stop)
    for m in targets:
            saved=OUT; T0=T[0]; res=[]
            try:
                snap0=copy.deepcopy((st,loc,this[2]))
                for start in (ti,fi):
                    OUT=[]
                    s_st,s_loc,s_this=copy.deepcopy(snap0)
                    this[2].clear(); this[2].update(s_this)
                    r=run(owner,code,None,this,state=(s_st,s_loc,start),stop=m)
                    if not (isinstance(r,tuple) and r and r[0]=='__merge__'): raise MergeError('no merge')
                    res.append((OUT,r[1],r[2],dict(this[2])))
                (o0,st0,loc0,th0),(o1,st1,loc1,th1)=res
                if len(st0)!=len(st1): raise MergeError('stack')
                decl=[]; memo=set()
                # one object graph: branch 0's, with branch 1's numbers merged in
                # Locals that don't merge are dead here: javac reuses a slot across scopes.
                for k in list(loc0):
                    if k not in loc1: del loc0[k]; continue
                    trial0,trial1,trial_decl=list(o0),list(o1),list(decl)
                    try:
                        loc0[k]=merge_vals(loc0[k],loc1[k],trial0,trial1,trial_decl,memo)
                        o0[:],o1[:],decl[:]=trial0,trial1,trial_decl
                    except MergeError:
                        del loc0[k]
                box0=('obj','__frame__',{'st':st0,'this':th0}); box1=('obj','__frame__',{'st':st1,'this':th1})
                merge_vals(box0,box1,o0,o1,decl,memo)
                box0[2]['loc']=loc0
                OUT=saved
                OUT.extend(decl)
                OUT.append('if %s {'%cond); OUT.extend('    '+x for x in o0); OUT.append('} else {'); OUT.extend('    '+x for x in o1); OUT.append('}')
                this[2].clear(); this[2].update(box0[2]['this'])
                if m==stop: return ('__merge__',box0[2]['st'],box0[2]['loc'])
                return run(owner,code,None,this,state=(box0[2]['st'],box0[2]['loc'],m),stop=stop)
            except MergeError as e:
                OUT=saved; T[0]=T[0]
    # tail duplication: both branches run on to the method's end
    saved=OUT; res=[]
    snap0=copy.deepcopy((st,loc,this[2]))
    for start in (ti,fi):
        OUT=[]
        s_st,s_loc,s_this=copy.deepcopy(snap0)
        this[2].clear(); this[2].update(s_this)
        r=run(owner,code,None,this,state=(s_st,s_loc,start),stop=stop)
        res.append((OUT,r,dict(this[2])))
    OUT=saved
    ret=res[0][1]
    a0,a1=res[0][1],res[1][1]
    numeric=lambda v: isinstance(v,(float,int,Sym))
    if isinstance(a0,tuple) and a0 and a0[0]=='__merge__':
        raise Exception('tail fork reached an outer merge point')
    if a0!=a1 and numeric(a0) and numeric(a1):
        T[0]+=1; rv='rv%d'%T[0]
        OUT.append('let %s: f32;'%rv)
        res[0][0].append('%s = %s;'%(rv,lit(a0))); res[1][0].append('%s = %s;'%(rv,lit(a1)))
        ret=Sym(rv)
    elif a0!=a1:
        raise Exception('fork with value return %r %r'%(str(a0)[:60],str(a1)[:60]))
    OUT.append('if %s {'%cond); OUT.extend('    '+x for x in res[0][0]); OUT.append('} else {'); OUT.extend('    '+x for x in res[1][0]); OUT.append('}')
    this[2].clear(); this[2].update(res[0][2])
    return ret
def run(owner, code, args, this, state=None, stop=None):
    pcmap={pc:i for i,(pc,_,_) in enumerate(code)}; steps=0
    if state: st,loc,i=state
    else:
        st=[]; loc={}; i=0
        for j,a in enumerate(args): loc[j]=a
    while i<len(code):
        if stop is not None and i==stop: return ('__merge__',st,loc)
        steps+=1
        if steps>100000: raise Exception('loop')
        pc,op,arg=code[i]; i+=1
        def cval():
            m=re.search(r'// (?:float|int|double|long) (-?[\d.E+-]+|NaN|Infinity)[fdl]?',arg)
            if m: return float(m.group(1))
            m=re.search(r'// String (.*)',arg); return m.group(1) if m else None
        if op in('ldc','ldc_w','ldc2_w'): st.append(cval())
        elif op.startswith(('fconst_','iconst_','dconst_','lconst_')): v=op.split('_')[1]; st.append(-1.0 if v=='m1' else float(v))
        elif op in('bipush','sipush'): st.append(float(arg.split()[0]))
        elif op=='aconst_null': st.append(None)
        elif re.match(r'[afidl]load(_\d)?$',op): n=int(op.split('_')[1]) if '_' in op else int(arg.split()[0]); st.append(loc.get(n))
        elif re.match(r'[afidl]store(_\d)?$',op):
            n=int(op.split('_')[1]) if '_' in op else int(arg.split()[0]); v=st.pop()
            if isinstance(v,Sym) and not re.match(r'^t\d+$|^[a-z_]+$',v): v=tmp(v)
            loc[n]=v
        elif op=='iinc': n,d=arg.split(','); loc[int(n)]=binop(loc[int(n)],float(d),'+')
        elif op=='dup': st.append(st[-1])
        elif op=='dup_x1': v=st.pop(); w=st.pop(); st+= [v,w,v]
        elif op=='dup2': st+= st[-2:]
        elif op=='dup_x2': v=st.pop(); w=st.pop(); x=st.pop(); st+=[v,x,w,v]
        elif op=='swap': v=st.pop(); w=st.pop(); st+=[v,w]
        elif op=='newarray': n=st.pop(); st.append([0.0]*int(n))
        elif op in('fastore','iastore','bastore'): v=st.pop(); idx=st.pop(); arr=st.pop(); arr[int(idx)]=v
        elif op in('faload','iaload','baload'): idx=st.pop(); arr=st.pop(); st.append(arr[int(idx)])
        elif op=='pop': st.pop()
        elif op in('fneg','ineg','dneg'): v=st.pop(); st.append(tmp('-%s'%lit(v)) if isinstance(v,Sym) else -v)
        elif op[1:] in('mul','add','sub','div','rem') and op[0] in 'fidl':
            b=st.pop();a=st.pop(); o={'mul':'*','add':'+','sub':'-','div':'/','rem':'%'}[op[1:]]
            if op[0] in 'il' and o in '/%' and (isinstance(a,Sym) or isinstance(b,Sym)):
                r=tmp('((%s as i32) %s (%s as i32)) as f32'%(lit(a),o,lit(b)))
            else:
                r=binop(a,b,o)
                if op[0] in 'il' and o=='/' and not isinstance(r,Sym): r=float(int(a/b))
                if op[0] in 'il' and o=='%' and not isinstance(r,Sym): r=float(int(__import__('math').fmod(a,b)))
            st.append(r)
        elif op in('iand','ior','ixor','ishl','ishr','iushr','land','lor'):
            b=st.pop();a=st.pop(); o={'iand':'&','land':'&','ior':'|','lor':'|','ixor':'^','ishl':'<<','ishr':'>>','iushr':'>>'}[op]
            if isinstance(a,Sym) or isinstance(b,Sym): st.append(tmp('((%s as i32) %s (%s as i32)) as f32'%(lit(a),o,lit(b))))
            else: st.append(float(eval('%d %s %d'%(int(a),o,int(b)))))
        elif op in('i2f','i2d','d2f','f2d','l2f','i2l','l2i','i2b','i2s','i2c'): pass
        elif op in('f2i','d2i'):
            v=st.pop(); st.append(tmp('(%s as i32) as f32'%lit(v)) if isinstance(v,Sym) else float(int(v)))
        elif op in('fcmpl','fcmpg','dcmpl','dcmpg','lcmp'):
            b=st.pop();a=st.pop()
            if isinstance(a,Sym) or isinstance(b,Sym): st.append(('cmp',a,b))
            else: st.append(float((a>b)-(a<b)))
        elif op.startswith('if'):
            t=int(arg.split()[0])
            if op in('ifnull','ifnonnull'): v=st.pop(); c=(v is None)==(op=='ifnull')
            elif op.startswith('if_acmp'): b=st.pop();a=st.pop(); c=(a is b or a==b)==(op=='if_acmpeq')
            elif op.startswith('if_icmp'):
                b=st.pop();a=st.pop()
                if isinstance(a,Sym) or isinstance(b,Sym):
                    rel={'eq':'==','ne':'!=','lt':'<','ge':'>=','gt':'>','le':'<='}[op[-2:]]
                    return fork(owner,code,'%s %s %s'%(lit(a),rel,lit(b)),pcmap[t],i,st,loc,this,stop)
                c={'ge':a>=b,'lt':a<b,'gt':a>b,'le':a<=b,'eq':a==b,'ne':a!=b}[op[-2:]]
            else:
                a=st.pop()
                if (isinstance(a,tuple) and a[0]=='cmp') or isinstance(a,Sym):
                    if isinstance(a,Sym): a=('cmp',a,0.0)
                    rel={'eq':'==','ne':'!=','lt':'<','ge':'>=','gt':'>','le':'<='}[op[2:]]
                    cond='%s %s %s'%(lit(a[1]),rel,lit(a[2]))
                    return fork(owner,code,cond,pcmap[t],i,st,loc,this,stop)
                c={'eq':a==0,'ne':a!=0,'lt':a<0,'ge':a>=0,'gt':a>0,'le':a<=0}[op[2:]]
            if c: i=pcmap[t]
        elif op=='goto': i=pcmap[int(arg.split()[0])]
        elif op=='getstatic':
            m=re.search(r'Field (\S+):',arg); LOG.add('getstatic '+m.group(1)); st.append(('static',m.group(1)))
        elif op=='getfield':
            obj=st.pop(); m=re.search(r'Field (?:([\w/$]+)\.)?(\w+):(\S+)',arg); fname=m.group(2); ty=m.group(3)
            if isinstance(obj,tuple) and obj[0]=='part':
                if fname in FIELD: st.append(tmp('p[%d][%d]'%(part_idx(obj[1]),FIELD[fname])))
                else: LOG.add('part field '+fname); st.append(1.0)
            elif isinstance(obj,tuple) and obj[0]=='obj':
                v=obj[2].get(fname)
                if v is None and ty=='F': v=Sym('pt') if fname=='partialTick' else 0.0
                st.append(v)
            else:
                if fname in ENT_SYM: st.append(Sym(ENT_SYM[fname])); continue
                LOG.add('getfield %s.%s %s on %r'%(m.group(1),fname,ty,obj if not isinstance(obj,tuple) else obj[0]))
                st.append(ENT_DEFAULTS.get(fname,0.0) if ty in 'FIDJZ' else None)
        elif op=='putfield':
            v=st.pop(); obj=st.pop(); m=re.search(r'Field (?:([\w/$]+)\.)?(\w+):(\S+)',arg); fname=m.group(2)
            if isinstance(obj,tuple) and obj[0]=='part':
                if fname in FIELD: OUT.append('p[%d][%d] = %s;'%(part_idx(obj[1]),FIELD[fname],lit(v)))
                else: LOG.add('put part field '+fname+'='+repr(v))
            elif isinstance(obj,tuple) and obj[0]=='obj': obj[2][fname]=v
            else: LOG.add('putfield other '+fname)
        elif op=='new': st.append(('obj',arg.split('class ')[-1].strip(),{}))
        elif op=='areturn' or op=='freturn' or op=='ireturn': return st.pop()
        elif op=='return': return None
        elif op=='checkcast' or op=='nop': pass
        elif op=='arraylength': v=st.pop(); st.append(float(len(v)) if isinstance(v,list) else 0.0)
        elif op=='invokedynamic':
            m=re.search(r'InvokeDynamic #(\d+):(\w+):\((.*?)\)',arg)
            argc=len(re.findall(r'\[*(?:L[^;]+;|[FIZDJBSC])',m.group(3))); a=[st.pop() for _ in range(argc)][::-1]
            if m.group(2)=='makeConcatWithConstants':
                rec=bsms(owner)[int(m.group(1))]; out=''
                for ch in rec:
                    if ch=='\x01': v=a.pop(0); out+=str(int(v)) if isinstance(v,float) else str(v)
                    else: out+=ch
                st.append(out)
            else: st.append(('lambda',))
        elif op.startswith('invoke'):
            m=re.search(r'Method (?:([\w/$]+)\.)?([\w<>"]+):\((.*?)\)(\S+)',arg)
            owner2,name,desc,ret=m.group(1) or owner,m.group(2).strip('"'),m.group(3),m.group(4)
            argc=len(re.findall(r'\[*(?:L[^;]+;|[FIZDJBSC])',desc)); a=[st.pop() for _ in range(argc)][::-1]
            recv=st.pop() if op!='invokestatic' else None
            full='%s.%s'%(owner2,name)
            r='__none__'
            if owner2=='net/minecraft/class_3532':
                if name=='method_15374': r=tmp('%s.sin()'%lit(a[0])) if isinstance(a[0],Sym) else __import__('math').sin(a[0])
                elif name=='method_15362': r=tmp('%s.cos()'%lit(a[0])) if isinstance(a[0],Sym) else __import__('math').cos(a[0])
                elif name=='method_15363': r=tmp('%s.clamp(%s, %s)'%(lit(a[0]),lit(a[1]),lit(a[2]))) if any(isinstance(x,Sym) for x in a) else min(max(a[0],a[1]),a[2])
                elif name=='method_16439': r=tmp('%s + (%s - %s) * %s'%(lit(a[1]),lit(a[2]),lit(a[1]),lit(a[0])))  # lerp(d, a, b)
                elif name=='method_15379': r=tmp('%s.abs()'%lit(a[0])) if isinstance(a[0],Sym) else abs(a[0])
                elif name=='method_15355': r=tmp('%s.sqrt()'%lit(a[0])) if isinstance(a[0],Sym) else a[0]**0.5
                elif name=='method_22450': r=tmp('{ let v = %s; v * v }'%lit(a[0])) if isinstance(a[0],Sym) else a[0]*a[0]
                elif name=='method_15393': r=tmp('wrap_deg(%s)'%lit(a[0]))
                else: raise Exception('Mth '+name)
            elif owner2=='java/util/Arrays' and name=='fill':
                for j in range(len(a[0])): a[0][j]=a[1]
            elif owner2=='java/lang/Math':
                if name in('sin','cos','abs','sqrt'): r=tmp('%s.%s()'%(lit(a[0]),name)) if isinstance(a[0],Sym) else getattr(__import__('math'),'fabs' if name=='abs' else name)(a[0])
                elif name=='min' or name=='max': r=tmp('%s.%s(%s)'%(lit(a[0]),name,lit(a[1]))) if any(isinstance(x,Sym) for x in a) else (min if name=='min' else max)(a)
                elif name=='pow': r=tmp('%s.powf(%s)'%(lit(a[0]),lit(a[1])))
                else: raise Exception('Math '+name)
            elif owner2=='net/minecraft/class_630':
                if name=='method_32086':
                    base=recv[1] if isinstance(recv,tuple) else 'EXT'
                    r=('part', base+'/'+a[0] if base else a[0])
                elif name in('method_2851',): # setPos
                    k=part_idx(recv[1]); OUT.extend(['p[%d][%d] = %s;'%(k,j,lit(a[j])) for j in range(3)])
                elif name=='method_33425': # setRotation
                    k=part_idx(recv[1]); OUT.extend(['p[%d][%d] = %s;'%(k,3+j,lit(a[j])) for j in range(3)])
                elif name=='method_41923': pass # resetPose? 
                elif name=='method_17138': # copyFrom
                    k=part_idx(recv[1]); s=part_idx(a[0][1]); OUT.append('p[%d] = p[%d];'%(k,s))
                else: LOG.add('ModelPart.'+name); r=None if ret!='V' else '__none__'
            elif isinstance(recv,tuple) and recv[0]=='obj' and recv[1]=='fuzs/mutantmonsters/client/animation/Animator':
                r=animator(recv[2],name,a)
            elif owner2=='fuzs/mutantmonsters/client/animation/Animator' and name=='addRotationAngle':
                k=part_idx(a[0][1])
                for j in range(3): OUT.append('p[%d][%d] += %s;'%(k,3+j,lit(a[1+j])))
            elif owner2=='fuzs/mutantmonsters/client/animation/Animator' and name=='resetAngles':
                for pp in (a[0] if isinstance(a[0],list) else [a[0]]):
                    k=part_idx(pp[1]); OUT.extend(['p[%d][%d] = 0.0;'%(k,3+j) for j in range(3)])
            elif owner2.startswith('fuzs/mutantmonsters/client/') and (op=='invokestatic' or (isinstance(recv,tuple) and recv[0]=='obj' and recv[1].startswith('fuzs/'))):
                cands=cls(owner2).get(name,[])
                if not cands: raise Exception('no method '+full)
                k,v=[c for c in cands if jdesc(c[0])==desc][0]
                r=run(owner2,v['code'],([] if v['static'] else [recv])+a,this)
                if r is None: r='__none__'
            elif name=='<init>': r='__none__'
            else:
                if name=='getAnimation': st.append(ANIM[0]); continue
                if name=='getAnimationTick': st.append(Sym('tick')); continue
                if name in ENT_SYM: st.append(Sym(ENT_SYM[name])); continue
                if name=='getActiveArm': USED_ARM[0]=True; st.append(float(ARM[0])); continue
                if name=='getHeldBlock': st.append(Sym('e[%d]'%(5+int(a[0])))); continue
                LOG.add('call %s%s on %s'%(full,'('+desc+')'+ret, 'static' if recv is None else (recv[0] if isinstance(recv,tuple) else repr(recv))))
                r=ENT_DEFAULTS.get(name, 0.0 if ret in('F','I','Z','D','J') else (('part','EXT') if ret=='Lnet/minecraft/class_630;' else None))
            if ret!='V' and r!='__none__': st.append(r)
        elif op=='anewarray': n=st.pop(); st.append([None]*int(n))
        elif op=='aastore': v=st.pop(); idx=st.pop(); arr=st.pop(); arr[int(idx)]=v
        elif op=='aaload': idx=st.pop(); arr=st.pop(); st.append(arr[int(idx)])
        elif op=='instanceof': st.pop(); st.append(0.0)
        else: raise Exception('op %s %s at %d'%(op,arg,pc))
    return None
def translate(model, args_names=('ws','wa','age','yaw','pitch'), defaults={}, anim=None):
    USED_ARM[0]=False
    ANIM[0]=anim or ('static','fuzs/mutantmonsters/world/entity/EntityAnimation.NONE')
    global OUT,PARTS,LOG,ENT_DEFAULTS
    OUT=[]; PARTS={}; LOG=set(); T[0]=0; ENT_DEFAULTS=defaults; FORKS[0]=0
    owner='fuzs/mutantmonsters/client/model/'+model
    this=('obj',owner,{})
    ctor=[v for k,v in cls(owner)['<init>']] if '<init>' in cls(owner) else None
    ms=cls(owner)
    init=[v for k,v in ms[model.split('/')[-1]] ] if model.split('/')[-1] in ms else None
    # javap prints constructors with the class name
    cand=[v for kk,v in ms['<init>'] if 'class_630' in kk]
    run(owner,cand[0]['code'],[this,('part','')],this)
    OUT=[]
    sa=[v for k,v in ms['setupAnim'] if 'fuzs.' in k.split('(')[1].split(',')[0] or k.split('(')[1].startswith('boolean')][0]
    run(owner,sa['code'],[this,('ent',)]+[Sym(n) for n in args_names],this)
    return OUT,PARTS,LOG
if __name__=='__main__':
    for m in sys.argv[2:]:
        try:
            out,parts,log=translate(m)
            print('==',m,len(out),'stmts'); print(json.dumps(parts)); print('\n'.join(sorted(log)))
        except Exception as e:
            import traceback; print('==',m,'ERR',e)
