"""Validate publishable metadata and local navigation across the static site."""
import json
from html.parser import HTMLParser
from pathlib import Path
import struct
import unittest
from urllib.parse import urljoin, urlsplit
import xml.etree.ElementTree as ET

PAGES = Path(__file__).resolve().parents[2] / 'pages'
ORIGIN = 'https://svb.miguel.ms/'


class Page(HTMLParser):
    def __init__(self, path):
        super().__init__()
        self.meta, self.links, self.ids, self.structured = {}, [], set(), []
        self.title = ''
        self.capture = None
        self.buffer = ''
        self.feed(path.read_text())

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if 'id' in attrs:
            self.ids.add(attrs['id'])
        if tag == 'meta':
            key = attrs.get('name', attrs.get('property'))
            if key:
                if key in self.meta:
                    raise AssertionError(f'Duplicate metadata: {key}')
                self.meta[key] = attrs.get('content')
        if tag == 'link' or tag == 'a':
            self.links.append(attrs)
        if tag == 'title' or (tag == 'script' and attrs.get('type') == 'application/ld+json'):
            self.capture = tag
            self.buffer = ''

    def handle_data(self, data):
        if self.capture:
            self.buffer += data

    def handle_endtag(self, tag):
        if tag == self.capture:
            if tag == 'title':
                self.title = self.buffer.strip()
            else:
                self.structured.append(json.loads(self.buffer))
            self.capture = None


class WebsiteSeoTests(unittest.TestCase):
    def setUp(self):
        self.pages = {p.name: Page(p) for p in PAGES.glob('*.html')}

    def test_metadata_agrees_with_canonical_and_structured_data(self):
        for name, page in self.pages.items():
            with self.subTest(page=name):
                canonical = ORIGIN + ('' if name == 'index.html' else name)
                self.assertEqual([x['href'] for x in page.links if x.get('rel') == 'canonical'], [canonical])
                self.assertIn('Speaker Volume Bridge', page.title)
                self.assertEqual(page.meta['robots'], 'index, follow')
                for prefix in ('og', 'twitter'):
                    self.assertEqual(page.meta[f'{prefix}:title'], page.title)
                    self.assertEqual(page.meta[f'{prefix}:description'], page.meta['description'])
                    self.assertEqual(page.meta[f'{prefix}:image'], ORIGIN + 'social-icon.png')
                    self.assertEqual(page.meta[f'{prefix}:image:alt'], 'Speaker Volume Bridge app icon')
                self.assertEqual(page.meta['og:url'], canonical)
                self.assertEqual(page.meta['og:site_name'], 'Speaker Volume Bridge')
                self.assertEqual(len(page.structured), 1)
                schema = page.structured[0]
                self.assertEqual(schema['url'], canonical)
                self.assertEqual(schema['description'], page.meta['description'])
                self.assertIn('Speaker Volume Bridge', schema['name'])
                width, height = struct.unpack('>II', (PAGES / 'social-icon.png').read_bytes()[16:24])
                self.assertEqual(int(page.meta['og:image:width']), width)
                self.assertEqual(int(page.meta['og:image:height']), height)

    def test_sitemap_covers_canonical_pages(self):
        urls = [e.text for e in ET.parse(PAGES / 'sitemap.xml').iter('{http://www.sitemaps.org/schemas/sitemap/0.9}loc')]
        expected = [ORIGIN + ('' if name == 'index.html' else name) for name in self.pages]
        self.assertCountEqual(urls, expected)
        self.assertIn('Sitemap: ' + ORIGIN + 'sitemap.xml', (PAGES / 'robots.txt').read_text())

    def test_local_links_and_fragments_exist(self):
        for name, page in self.pages.items():
            for link in page.links:
                href = link.get('href', '')
                target = urlsplit(urljoin(ORIGIN + name, href))
                if target.netloc != urlsplit(ORIGIN).netloc:
                    continue
                path = target.path.lstrip('/') or 'index.html'
                with self.subTest(page=name, href=href):
                    self.assertTrue((PAGES / path).is_file(), path)
                    if target.fragment and path in self.pages:
                        self.assertIn(target.fragment, self.pages[path].ids)


if __name__ == '__main__':
    unittest.main()
