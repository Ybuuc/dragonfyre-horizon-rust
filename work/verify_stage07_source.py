"""Verify quest identifiers and the exact exchange fields against original FTB data."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from read_stage07_quests import SOURCE, chapters

ROOT = Path(__file__).resolve().parents[1]
RUBY, CORE, EXCHANGE = '5AFA5CB101E31BE4', '288608ED892DC5EB', '2AE651270FD58200'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--chapters', type=Path, default=SOURCE)
    args = parser.parse_args()
    source = dict(chapters(args.chapters))
    original = {q['id']:(path,q) for path,qs in source.items() for q in qs}
    count = sum(len(qs) for qs in source.values())
    rewards = sum(len(q.get('rewards',[])) for path,q in original.values())
    assert len(original) == count == 461 and rewards == 803
    engine = ROOT/'native/dragonfyre-quests/engine/src'
    pattern = r'Quest \{ id: 0x([0-9A-F]+), dependencies: vec!\[(.*?)\], tasks: vec!\[(.*?)\]'
    built = {i:(re.findall('0x([0-9A-F]+)',d), re.findall('0x([0-9A-F]+)',t))
             for i,d,t in re.findall(pattern,(engine/'catalog.rs').read_text())}
    assert set(original) == set(built)
    assert all(built[i] == (q.get('dependencies',[]),[t['id'] for t in q['tasks']]) for i,(_,q) in original.items())
    result = {'quest_count':count,'reward_count':rewards,'quest_ids_dependencies_tasks_match_stage06':True,'chain':[]}
    definitions = (engine/'economy_catalog.rs').read_text()
    for qid,taskid,item,n,consume,rewardid,reward,m in [
        (RUBY,'022397B35A0E22E3','cisco_mod:radiant_ruby',1,False,'7940159F83CA50B6','cisco_mod:champion_coin',3),
        (CORE,'4F241D893DBF361F','cisco_mod:divine_core',1,False,'600FF15BCE3E202C','cisco_mod:champion_coin',2),
        (EXCHANGE,'670F3886215AD741','cisco_mod:champion_coin',2,True,'1AAF2DB7F3B1A419','farmersdelight:honey_glazed_ham',16),
    ]:
        path,q = original[qid]
        assert q.get('dependencies',[]) == [] and bool(q.get('can_repeat',False)) == consume
        task, = q['tasks']
        prize, = q['rewards']
        assert (task['id'],task['type'],task['item'],task.get('count',1),task.get('consume_items',False)) == (taskid,'item',item,n,consume)
        assert (prize['id'],prize['type'],prize['item'],prize.get('count',1)) == (rewardid,'item',reward,m)
        costs = f'(0x{taskid}, "{item}".into(), {n})' if consume else ''
        assert f'Definition {{ quest: 0x{qid}, costs: vec![{costs}], items: vec![("{reward}".into(), {m})], levels: 0, blocked: None }}' in definitions
        result['chain'].append(dict(chapter=path.name,sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
            quest=qid,dependencies=[],repeat=consume,task=task,reward=prize))
    out = ROOT/'outputs/native-stage-07'
    out.mkdir(parents=True,exist_ok=True)
    (out/'source-verification.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    print(f'PASS: {count} quests, {rewards} rewards; three original quests and their precise costs and rewards')


if __name__=='__main__':
    main()
