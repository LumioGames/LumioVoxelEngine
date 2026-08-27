#!/usr/bin/env python3
from __future__ import annotations
import hashlib, json, re, sys
from pathlib import Path

root = Path(__file__).resolve().parent
errors = []
warnings = []
required = [
  'FRAMEWORK_IMPLEMENTATION_DESIGN.md','00_SOURCE_INVENTORY.md','01_REQUIREMENTS_TRACEABILITY.md',
  'DECISION_GATES.md','TASK_CARDS.md','task-cards.json','VERIFICATION_MATRIX.md','KNOWN_GAPS.md'
]
for f in required:
    if not (root/f).is_file(): errors.append(f'missing required file: {f}')
packages = sorted((root/'packages').glob('*.md'))
if len(packages) != 10: errors.append(f'expected 10 package designs, found {len(packages)}')
sections = ['## 1. Purpose and boundary','## 2. Physical placement','Verification surface','Acceptance criteria']
for p in packages:
    text=p.read_text(encoding='utf-8')
    for s in sections:
        if s.lower() not in text.lower(): errors.append(f'{p.name}: missing section marker {s}')
    if re.search(r'#\[repr\(C\)\]|extern\s+"C"\s+fn|pub\s+struct\s+Lge', text):
        errors.append(f'{p.name}: appears to declare public ABI code')

tasks=json.loads((root/'task-cards.json').read_text(encoding='utf-8'))
ids=[t['id'] for t in tasks]
if len(ids)!=len(set(ids)): errors.append('duplicate task ID')
owned={}
for t in tasks:
    for f in t['owned_files']:
        if f in owned: errors.append(f'file ownership collision: {f} in {owned[f]} and {t["id"]}')
        owned[f]=t['id']
    for d in t['dependencies']:
        if d not in ids: errors.append(f'{t["id"]}: missing dependency {d}')
# cycle check
edges={t['id']:t['dependencies'] for t in tasks}
visiting=set(); visited=set()
def dfs(n):
    if n in visiting: errors.append(f'dependency cycle at {n}'); return
    if n in visited:return
    visiting.add(n)
    for d in edges.get(n,[]): dfs(d)
    visiting.remove(n); visited.add(n)
for n in edges: dfs(n)
# decision status and coverage
text=(root/'DECISION_GATES.md').read_text(encoding='utf-8')
for i in range(1,9):
    d=f'VOX-D-{i:03d}'
    if d not in text: errors.append(f'missing decision gate {d}')
if text.count('`unapproved`') < 8: errors.append('not all eight decision gates explicitly unapproved')
# No production source/project files inside design package.
for p in root.rglob('*'):
    if p.is_file() and p.suffix in {'.rs','.cs'}: errors.append(f'production source present in design package: {p.relative_to(root)}')
    if p.name in {'Cargo.toml','Cargo.lock'} or p.suffix=='.csproj': errors.append(f'project/build file present: {p.relative_to(root)}')
# Markdown relative link existence (simple local links only).
link_re=re.compile(r'\[[^\]]*\]\(([^)#]+)(?:#[^)]+)?\)')
for p in root.rglob('*.md'):
    txt=p.read_text(encoding='utf-8')
    for target in link_re.findall(txt):
        if '://' in target or target.startswith('mailto:'): continue
        dest=(p.parent/target).resolve()
        if not dest.exists(): errors.append(f'broken link in {p.relative_to(root)}: {target}')
# Warn, do not silently pass, on unresolved frozen crate alias.
inv=(root/'00_SOURCE_INVENTORY.md').read_text(encoding='utf-8')
if 'SOURCE_CRATE_MAP_REQUIRED' in inv: warnings.append('one or more physical crate aliases require W0 source resolution')
report={'status':'PASS' if not errors else 'FAIL','errors':errors,'warnings':warnings,'package_count':len(packages),'task_count':len(tasks),'owned_file_count':len(owned)}
(root/'validation-result.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
print(json.dumps(report,indent=2))
sys.exit(1 if errors else 0)
