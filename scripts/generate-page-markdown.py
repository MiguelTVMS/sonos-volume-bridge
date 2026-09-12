"""Generate readable Markdown counterparts from published HTML content."""
import argparse
import re
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import urljoin

ROOT = Path(__file__).resolve().parents[1]
ORIGIN = 'https://svb.miguel.ms/'
VOID = {'area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'param', 'source', 'track', 'wbr'}


class Document(HTMLParser):
    def __init__(self):
        super().__init__()
        self.root = ['document', {}, []]
        self.stack = [self.root]

    def handle_starttag(self, tag, attrs):
        node = [tag, dict(attrs), []]
        self.stack[-1][2].append(node)
        if tag not in VOID:
            self.stack.append(node)

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag not in VOID:
            self.handle_endtag(tag)

    def handle_endtag(self, tag):
        for index in range(len(self.stack) - 1, 0, -1):
            if self.stack[index][0] == tag:
                del self.stack[index:]
                break

    def handle_data(self, data):
        self.stack[-1][2].append(data)


def render(node, base):
    if isinstance(node, str):
        return re.sub(r'\s+', ' ', node)
    tag, attrs, children = node
    if tag in {'svg', 'script', 'style', 'nav', 'header', 'button', 'noscript'} or attrs.get('aria-hidden') == 'true' or attrs.get('role') == 'img':
        return ''
    body = ''.join(render(child, base) for child in children).strip()
    if tag == 'br':
        return ' '
    if not body:
        return ''
    if tag in {'h1', 'h2', 'h3', 'h4'}:
        return '\n\n' + '#' * int(tag[1]) + ' ' + body + '\n\n'
    if tag == 'a':
        return '[' + body + '](' + urljoin(base, attrs.get('href', '')) + ') '
    if tag in {'strong', 'b'}:
        return '**' + body + '** '
    if tag == 'code':
        return '`' + body + '`'
    if tag == 'li':
        return '\n- ' + re.sub(r'\s+', ' ', body) + '\n'
    if tag in {'p', 'div', 'section', 'article', 'footer', 'ul', 'ol', 'small'}:
        return '\n\n' + body + '\n\n'
    return body + ' '


def generate(source):
    document = Document()
    document.feed(source.read_text())
    def find(node):
        if isinstance(node, str):
            return []
        if node[0] in {'main', 'footer'}:
            return [node]
        return [match for child in node[2] for match in find(child)]
    base = ORIGIN if source.name == 'index.html' else urljoin(ORIGIN, source.name)
    text = '\n\n'.join(render(node, base) for node in find(document.root))
    text = re.sub(r'\n[ \t]+', '\n', text)
    text = re.sub(r'[ \t]+\n', '\n', text)
    text = re.sub(r'\n{3,}', '\n\n', text).strip()
    return '<!-- Generated from ' + source.name + '; edit the HTML source. -->\n\n' + text + '\n'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    for source in sorted((ROOT / 'pages').glob('*.html')):
        target = source.with_suffix('.md')
        content = generate(source)
        if args.check:
            if not target.exists() or target.read_text() != content:
                raise SystemExit(f'Markdown is missing or stale: {target.name}')
        else:
            target.write_text(content)


if __name__ == '__main__':
    main()
