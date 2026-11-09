import sys,json
sys.argv=['x','cls']
exec(open('../mmtools/anim.py').read().split("if __name__=='__main__':")[0])
L=json.load(open('layers.json'))
MODELS=[('mutant_zombie','MutantZombieModel'),('mutant_skeleton','MutantSkeletonModel'),('mutant_creeper','MutantCreeperModel'),
 ('mutant_enderman','MutantEndermanModel'),('mutant_snow_golem','MutantSnowGolemModel'),('spider_pig','SpiderPigModel'),('creeper_minion','CreeperMinionModel')]
def f(x): 
    s=repr(float(x)); return s if 'e' not in s else '%.8f'%x
out=['// Generated from the Mutant Monsters jar\'s model classes by .local/setup/mutantmonsters_gen.py: the',
 '// layers (createBodyLayer) and the idle and walk poses (setupAnim with no attack under way).',
 'use horizon_client::mobs::model_layers::{CubeDeformation, CubeListBuilder, LayerDefinition, MeshDefinition, PartId, PartPose};','']
def mesh_code(layer):
    lines=['    let mut mesh = MeshDefinition::new();','    let root = mesh.root();']
    n=[0]
    def walk(p,parent):
        n[0]+=1; var='p%d'%n[0]
        b='CubeListBuilder::create()'
        for c in p['cubes']:
            b+='.tex_offs(%d, %d).set_mirror(%s).add_box_with([%s, %s, %s], [%s, %s, %s], CubeDeformation::xyz(%s, %s, %s))'%(c['uv'][0],c['uv'][1],'true' if c['mirror'] else 'false',*map(f,c['origin']),*map(f,c['size']),*map(f,c['grow']))
        po=p['pose'] or {'offset':[0,0,0],'rot':[0,0,0]}
        lines.append('    let %s: PartId = mesh.add_or_replace_child(%s, "%s", %s, PartPose::offset_and_rotation(%s, %s, %s, %s, %s, %s));'%(var,parent,p['name'],b,*map(f,po['offset']),*map(f,po['rot'])))
        for c in p['children']: walk(c,var)
    for c in layer['root']['children']: walk(c,'root')
    lines.append('    let _ = root;')
    return lines
out.append('pub(super) fn layers(layers: &mut Vec<(String, LayerDefinition)>) {')
for key in ['mutant_zombie','mutant_skeleton','mutant_creeper','mutant_creeper_armor','mutant_enderman','mutant_snow_golem','mutant_snow_golem_head','spider_pig','creeper_minion','creeper_minion_armor','mutant_crossbow','mutant_arrow','endersoul_fragment','mutant_skeleton_part']:
    lay=L[key]; out.append('    {'); out+=['    '+x for x in mesh_code(lay)]
    name=key.replace('_armor','#armor').replace('_head','#head') if key.endswith(('_armor','_head')) else key+'#main'
    out.append('        layers.push(("mutantmonsters:%s".into(), LayerDefinition::create(mesh, %d, %d)));'%(name,lay['w'],lay['h'])); out.append('    }')
out.append('}'); out.append('')
Z='fuzs/mutantmonsters/world/entity/mutant/'
ANIMS={'mutant_zombie':['MutantZombie.SLAM_GROUND_ANIMATION','MutantZombie.ROAR_ANIMATION','MutantZombie.THROW_ANIMATION'],
 'mutant_skeleton':['MutantSkeleton.MELEE_ANIMATION','MutantSkeleton.SHOOT_ANIMATION','MutantSkeleton.MULTI_SHOT_ANIMATION','MutantSkeleton.CONSTRICT_RIBS_ANIMATION'],
 'mutant_enderman':['MutantEnderman.MELEE_ANIMATION','MutantEnderman.THROW_ANIMATION','MutantEnderman.SCREAM_ANIMATION','MutantEnderman.TELESMASH_ANIMATION','MutantEnderman.DEATH_ANIMATION','MutantEnderman.STARE_ANIMATION','MutantEnderman.TELEPORT_ANIMATION','MutantEnderman.CLONE_ANIMATION']}
for key,model in MODELS:
    anims=[None]+ANIMS.get(key,[])
    parts_all={}
    bodies=[]
    jobs=[]
    for a in anims:
        ARM[0]=0
        code,parts,log=translate(model,anim=('static',Z+a) if a else None)
        if not USED_ARM[0]:
            jobs.append((a,None,code,parts)); continue
        for arm in range(5):
            ARM[0]=arm
            code,parts,log=translate(model,anim=('static',Z+a) if a else None)
            jobs.append((a,arm,code,parts))
    for a,arm,code,parts in jobs:
        # one index space per model: renumber this translation's parts into parts_all
        remap={}
        for path,i in parts.items():
            if path not in parts_all: parts_all[path]=len(parts_all)
            remap[i]=parts_all[path]
        import re as _re
        code=[_re.sub(r'p\[(\d+)\]',lambda mm:'p[%d]'%remap[int(mm.group(1))],l) for l in code]
        bodies.append((a,arm,code))
    paths=sorted(parts_all,key=lambda k:parts_all[k])
    out.append('/// The parts `anim_%s` poses, by path from the root.'%key)
    out.append('pub(super) const %s_PARTS: [&str; %d] = [%s];'%(key.upper(),len(paths),', '.join('"%s"'%p for p in paths)))
    out.append('/// Its animations (`EntityAnimation`s) by number, from 1; 0 is none.')
    out.append('pub(super) const %s_ANIMS: [&str; %d] = [%s];'%(key.upper(),len(anims)-1,', '.join('"%s"'%a.split('.')[1] for a in anims[1:])))
    out.append('#[allow(unused_variables, unused_mut, unused_parens, clippy::all)]')
    out.append('pub(super) fn anim_%s(p: &mut [[f32; 10]], ws: f32, wa: f32, age: f32, yaw: f32, pitch: f32, pt: f32, anim: u8, tick: f32, e: &[f32; 12]) {'%key)
    out.append('    match (anim, e[3] as u8) {')
    for (a,arm,code) in bodies:
        n=anims.index(a)
        out.append('        (%d, %s) => {'%(n,'_' if arm is None else arm))
        out+=['            '+l for l in code]
        out.append('        }')
    out.append('        _ => {}')
    out.append('    }')
    out.append('}'); out.append('')
    print(key,[len(c) for _,_,c in bodies],file=sys.stderr)
open('generated.rs','w').write('\n'.join(out)+'\n')
