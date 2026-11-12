"""Export supported Feishu XML to a self-contained Markdown package (stdlib only)."""
import argparse
import collections
import json
import re
from pathlib import Path
from urllib.parse import unquote, urlsplit
import xml.etree.ElementTree as ET

MEDIA = {'.png', '.jpg', '.jpeg', '.gif', '.webp'}


def read_root(path):
    source = Path(path).read_text(encoding='utf-8-sig')
    if Path(path).suffix.lower() == '.json':
        record = json.loads(source)
        if not record.get('ok'):
            raise ValueError('Fetch was not successful')
        source = record['data']['document']['content']
    source = re.sub(r'<br\s*/?>', '\n', source)
    return ET.fromstring(source if source.lstrip().startswith('<root>') else '<root>' + source + '</root>')


def plain(node):
    return ''.join(node.itertext())


def inline(node):
    result = node.text or ''
    for child in node:
        if child.tag == 'a':
            result += '[' + plain(child) + '](' + child.attrib['href'] + ')'
        elif child.tag in {'b', 'em', 'u', 'del', 'span', 'code'}:
            result += inline(child)
        else:
            raise ValueError('Unsupported inline element: ' + child.tag)
        result += child.tail or ''
    return result


def export(input_path, package, output_dir=None, appendix_title=None):
    package = Path(package).resolve()
    output = Path(output_dir).resolve() if output_dir else package
    root = read_root(input_path)
    def container(node, allowed):
        if (node.text or '').strip() or any((c.tail or '').strip() for c in node):
            raise ValueError('Unexpected direct text in container: ' + node.tag)
        if any(c.tag not in allowed for c in node):
            raise ValueError('Unsupported child in container: ' + node.tag)

    container(root, {'title', 'p', 'pre', 'img', 'table', 'figure', 'source'} | {'h'+str(i) for i in range(1,10)})
    by_name = collections.defaultdict(list)
    media = set()
    all_paths = set()
    for path in package.rglob('*'):
        if path.is_symlink():
            raise ValueError('Symlink in package: ' + str(path))
        if path.is_file():
            rel = path.relative_to(package).as_posix()
            if rel.casefold() in all_paths:
                raise ValueError('Case-insensitive duplicate path: ' + rel)
            all_paths.add(rel.casefold())
            if path.suffix.lower() in MEDIA:
                by_name[path.name].append(rel)
                media.add(rel)
    used = []

    def render(node):
        tag = node.tag  # Element name, never the media's `name` attribute.
        if tag == 'title':
            return '# ' + inline(node) + '\n\n'
        if re.fullmatch(r'h[1-9]', tag):
            return '#' * (int(tag[1]) + 1) + ' ' + inline(node) + '\n\n'
        if tag == 'p':
            value = inline(node).strip()
            return value + '\n\n' if value else ''
        if tag == 'pre':
            container(node, {'code'})
            code = node.find('code')
            if code is None or len(node) != 1 or len(code):
                raise ValueError('Code block must contain one plain code element')
            value = plain(code).replace('\r\n', '\n')
            fence = '`' * max(3, max((len(x) + 1 for x in re.findall(r'`+', value)), default=3))
            return fence + node.get('lang', '') + '\n' + value + '\n' + fence + '\n\n'
        if tag == 'img':
            container(node, set())
            name = node.get('name')
            if not name and node.get('path'):
                name = Path(node.get('path')).name
            candidates = by_name.get(name, [])
            if len(candidates) != 1:
                raise ValueError(f'Media must resolve uniquely: {name}: {candidates}')
            rel = candidates[0]
            used.append(rel)
            alt = (node.get('caption') or name).strip().replace('[', '').replace(']', '').replace('\n', ' ')
            return '![' + alt + '](' + rel + ')\n\n'
        if tag == 'table':
            container(node, {'thead', 'tbody', 'tr', 'colgroup'})
            for group in node:
                if group.tag in {'thead', 'tbody'}:
                    container(group, {'tr'})
                elif group.tag == 'colgroup':
                    container(group, {'col'})
                    for col in group:
                        container(col, set())
            rows = []
            for row in node.findall('.//tr'):
                container(row, {'th', 'td'})
                cells = []
                for cell in row:
                    if cell.tag not in {'th', 'td'}:
                        raise ValueError('Unsupported table cell: ' + cell.tag)
                    if any(int(cell.get(k, '1')) != 1 for k in ('rowspan', 'colspan')):
                        raise ValueError('Merged cells need explicit conversion')
                    container(cell, {'p'})
                    parts = []
                    for paragraph in cell:
                        if paragraph.tag != 'p':
                            raise ValueError('Unsupported content inside table cell')
                        parts.append(inline(paragraph).strip())
                    cells.append('<br>'.join(parts).replace('|', '\\|').replace('\n', '<br>'))
                rows.append(cells)
            if not rows or len({len(r) for r in rows}) != 1:
                raise ValueError('Empty or ragged table')
            lines = ['| ' + ' | '.join(r) + ' |' for r in rows]
            lines.insert(1, '| ' + ' | '.join(['---'] * len(rows[0])) + ' |')
            return '\n'.join(lines) + '\n\n'
        if tag in {'figure', 'source'}:
            # The online ZIP attachment is intentionally not embedded inside itself.
            container(node, set() if tag == 'source' else {'source'})
            sources = [node] if tag == 'source' else list(node)
            for source in sources:
                container(source, set())
            if sources and all(n.tag == 'source' and n.get('name', '').endswith('.zip') for n in sources):
                return ''
            raise ValueError('Non-ZIP attachment needs explicit package mapping')
        raise ValueError('Unsupported block: ' + tag)

    texts = {'gameplay.md': ''}
    current = 'gameplay.md'
    found = False
    for node in root:
        if appendix_title and re.fullmatch(r'h[1-9]', node.tag) and plain(node) == appendix_title:
            if found:
                raise ValueError('Duplicate appendix heading')
            current = 'effects-reference.md'
            texts[current] = ''
            texts['gameplay.md'] += '[完整特效配置、公式与原素材](effects-reference.md)。\n\n'
            found = True
        texts[current] += render(node)
    if appendix_title and not found:
        raise ValueError('Requested appendix was not found')
    if set(used) != media:
        raise ValueError('Unreferenced package media: ' + repr(sorted(media - set(used))))
    image_count = len(root.findall('.//img'))
    if len(used) != image_count:
        raise ValueError('XML image coverage mismatch')
    generated_images = sum(len(re.findall(r'!\[[^\]]*\]\(', s)) for s in texts.values())
    if generated_images != image_count:
        raise ValueError('Markdown image coverage mismatch')
    for text in texts.values():
        for target in re.findall(r'\]\(([^)]+)\)', text):
            if urlsplit(target).scheme in {'https', 'http'}:
                continue
            rel = unquote(target.split('#', 1)[0])
            path = (package / rel).resolve()
            if not path.is_relative_to(package) or ':' in rel or '\\' in rel:
                raise ValueError('Unsafe relative link: ' + target)
            if rel not in texts and not path.is_file():
                raise ValueError('Unresolved link: ' + target)
    output.mkdir(parents=True, exist_ok=True)
    for name, value in texts.items():
        (output / name).write_text(value, encoding='utf-8', newline='\n')
    return {'xmlImages': image_count, 'markdownImages': generated_images,
            'referencedMedia': sorted(set(used)),
            'documents': {n: {'characters': len(t), 'lines': len(t.splitlines())} for n, t in texts.items()}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', required=True)
    parser.add_argument('--package', required=True)
    parser.add_argument('--output-dir')
    parser.add_argument('--appendix-title')
    parser.add_argument('--report')
    args = parser.parse_args()
    report = export(args.input, args.package, args.output_dir, args.appendix_title)
    text = json.dumps(report, ensure_ascii=False, indent=2)
    if args.report:
        Path(args.report).write_text(text, encoding='utf-8')
    print(text)


if __name__ == '__main__':
    main()
