# Copyright 2026 Janek Bevendorff
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

import os

import pytest
from fastwarc.warc import *
from resiliparse.parse.html import *
from resiliparse.extract.html2text import *

DATA_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..', 'test-data'))

html = """<!doctype html>
<head>
    <title>Foo</title>
    <meta charset="utf-8">
    <style>* { margin: 0; }</style>
</head>
<body>
    <section id="wrapper">
        <nav>
            <ul>
                <li>Nav 1</li>        
                <li>
                    <p>Nav 2</p>
                    <ul>
                        <li><p>Nav 3</p></li>
                    </ul>
                </li>
            </ul>
        </nav>
        <main>
            foo <a href="#foo" hidden>bar</a>

            <p>baz<br>bar</p>

            <button aria-hidden="true">Click here</button>
            <input type="hidden" value="foo">
            <input type="text" value="Some text" placeholder="Insert text">
            <input type="text" placeholder="Insert text">
            <img src="" alt="Some image">
            <object data="" class="some-class hidden">Cannot display object</object>
        </main>
        <script language="vbscript" type="text/vbscript">MsgBox("Hello World!")</script>
        <noscript>Sorry, your browser doesn't support VB Script!</noscript>
        <div><div><div><footer id="global-footer">
            Copyright (C) 2021 Foo Bar
        </footer></div></div></div>
    </section>
</body>
</html>"""

tree = HTMLTree.parse(html)


def test_basic_extraction():
    for inp in [html, tree]:  # Run twice, once with string input, once with HTML tree
        assert extract_plain_text(inp, alt_texts=False, preserve_formatting=False) == \
               "Nav 1 Nav 2 Nav 3 foo bar baz bar Copyright (C) 2021 Foo Bar"

        assert extract_plain_text(inp, alt_texts=False, list_bullets=False) == """\
  Nav 1

  Nav 2

    Nav 3

foo bar

baz
bar

Copyright (C) 2021 Foo Bar"""

        assert extract_plain_text(inp, alt_texts=False) == """\
  \u2022 Nav 1

  \u2022 Nav 2

    \u2022 Nav 3

foo bar

baz
bar

Copyright (C) 2021 Foo Bar"""

        assert extract_plain_text(inp, alt_texts=False, preserve_formatting='minimal_html') == """\
<ul>
  <li>Nav 1</li>
  <li>

  <p>Nav 2  </p>

  <ul>
    <li>

    <p>Nav 3    </p></li>
  </ul></li>
</ul>
foo bar

<p>baz<br>
bar</p>

Copyright (C) 2021 Foo Bar"""

        assert extract_plain_text(inp, alt_texts=True, preserve_formatting='minimal_html') == """\
<ul>
  <li>Nav 1</li>
  <li>

  <p>Nav 2  </p>

  <ul>
    <li>

    <p>Nav 3    </p></li>
  </ul></li>
</ul>
foo bar

<p>baz<br>
bar</p>

Some image Cannot display object
Copyright (C) 2021 Foo Bar"""

        assert extract_plain_text(inp, alt_texts=True, preserve_formatting='minimal_html', list_bullets=False) == """\
Nav 1

<p>Nav 2</p>

<p>Nav 3</p>

foo bar

<p>baz<br>
bar</p>

Some image Cannot display object
Copyright (C) 2021 Foo Bar"""

    with pytest.raises(TypeError):
        extract_plain_text(123)


def test_alt_text_extraction():
    assert extract_plain_text(tree, alt_texts=True) == """\
  \u2022 Nav 1

  \u2022 Nav 2

    \u2022 Nav 3

foo bar

baz
bar

Some image Cannot display object
Copyright (C) 2021 Foo Bar"""


def test_link_href_extraction():
    assert extract_plain_text(tree, alt_texts=False, links=True) == """\
  \u2022 Nav 1

  \u2022 Nav 2

    \u2022 Nav 3

foo bar (#foo)

baz
bar

Copyright (C) 2021 Foo Bar"""


def test_form_field_extraction():
    assert extract_plain_text(tree, alt_texts=False, form_fields=True) == """\
  \u2022 Nav 1

  \u2022 Nav 2

    \u2022 Nav 3

foo bar

baz
bar

[ Click here ] [ Some text ] [ Insert text ]
Copyright (C) 2021 Foo Bar"""


def test_noscript_extraction():
    assert extract_plain_text(tree, alt_texts=False, noscript=True) == """\
  \u2022 Nav 1

  \u2022 Nav 2

    \u2022 Nav 3

foo bar

baz
bar

Sorry, your browser doesn't support VB Script!
Copyright (C) 2021 Foo Bar"""


def test_main_content_extraction():
    assert extract_plain_text(tree, alt_texts=False, main_content=True) == \
           "foo\n\nbaz\nbar"
    assert extract_plain_text(tree, alt_texts=True, main_content=True) == \
           "foo\n\nbaz\nbar\n\nSome image"
    assert extract_plain_text(tree, alt_texts=False, main_content=True, form_fields=True) == \
           "foo\n\nbaz\nbar\n\n[ Some text ] [ Insert text ]"


# Legacy extraction cases, with class-only comment filtering corrected when comments=False.
LEGACY_COMPATIBILITY_HTML = """<html><body>
<header class="page-header">Header</header>
<main class="main-content" role="main">
<h1>Title</h1>
<p>Visible <a href="/x">link</a>!</p>
<p>■</p><p>\ue123</p>
<ul><li>One</li><li>Two</li></ul>
<img alt="Picture">
<form><input type="text" value="Search"><button>Go</button></form>
<noscript>Fallback</noscript>
<div><div>
<div class="hidden">Hidden</div>
<div class="skip">Skip</div>
<div class="post-meta">By me</div>
<div class="comments">Comments</div>
</div></div>
</main>
<footer class="global-footer">Footer</footer>
</body></html>"""


@pytest.mark.parametrize(('options', 'expected'), [
    ({},
     "Header\n\nTitle\n\nVisible link!\n\n■\n\n\ue123\n\n  • One\n  • Two\nPicture\nHidden\nSkip\nBy me\nComments\nFooter"),
    ({'preserve_formatting': False},
     "Header Title Visible link! ■ \ue123 One Two Picture Hidden Skip By me Comments Footer"),
    ({'preserve_formatting': 'minimal_html'},
     "Header\n\n<h1>Title</h1>\n\n<p>Visible link!</p>\n\n<p>■</p>\n\n<p>\ue123</p>\n\n<ul>\n  <li>One</li>\n  <li>Two</li>\n</ul>\nPicture\nHidden\nSkip\nBy me\nComments\nFooter"),
    ({'main_content': True}, "Header\n\nTitle\n\nVisible link!\n\nPicture\nBy me\nComments"),
    ({'main_content': True, 'comments': False, 'post_meta': False, 'hidden_elements': True,
      'skip_elements': ['.skip']}, "Header\n\nTitle\n\nVisible link!\n\nPicture\nHidden"),
    ({'links': True, 'form_fields': True, 'noscript': True, 'list_bullets': False},
     "Header\n\nTitle\n\nVisible link (/x)!\n\n■\n\n\ue123\n\n  One\n  Two\nPicture\n[ Search ] [ Go ]\nFallback\nHidden\nSkip\nBy me\nComments\nFooter"),
    ({'alt_texts': False},
     "Header\n\nTitle\n\nVisible link!\n\n■\n\n\ue123\n\n  • One\n  • Two\nHidden\nSkip\nBy me\nComments\nFooter"),
])
def test_synthetic_legacy_compatibility(options, expected):
    actual = extract_plain_text(LEGACY_COMPATIBILITY_HTML, **options)
    assert actual.encode('utf-8') == expected.encode('utf-8')


SYNTHETIC_EDGE_CASE_HTML = '''<body><main>
<p>Alpha <span>beta</span>   gamma<br>delta</p>
<pre> x
  y </pre>
<ol><li>First<ul><li>Nested</li></ul></li><li>Second</li></ol>
<p><a href="/a?x=1&amp;y=2">Link</a></p>
<img alt="Diagram">
<input type="text" placeholder="Find">
<input type="checkbox" value="ignored">
<textarea>Note</textarea>
<div class="skip-me">Skip</div>
</main></body>'''


@pytest.mark.parametrize(('options', 'expected'), [
    ({}, "Alpha beta gamma\ndelta\n\n x\n  y\n  1. First\n    • Nested\n  2. Second\n\nLink\n\nDiagram\nSkip"),
    ({'preserve_formatting': False},
     "Alpha beta gamma delta x y First Nested Second Link Diagram Skip"),
    ({'list_bullets': False},
     "Alpha beta gamma\ndelta\n\n x\n  y\n  First\n    Nested\n  Second\n\nLink\n\nDiagram\nSkip"),
    ({'preserve_formatting': 'minimal_html'},
     "<p>Alpha beta gamma<br>\ndelta</p>\n\n<pre> x\n  y </pre>\n<ol>\n  <li>First\n  <ul>\n    <li>Nested</li>\n  </ul></li>\n  <li>Second</li>\n</ol>\n\n<p>Link</p>\n\nDiagram\nSkip"),
    ({'preserve_formatting': 'minimal_html', 'links': True},
     "<p>Alpha beta gamma<br>\ndelta</p>\n\n<pre> x\n  y </pre>\n<ol>\n  <li>First\n  <ul>\n    <li>Nested</li>\n  </ul></li>\n  <li>Second</li>\n</ol>\n\n<p><a href=\"/a?x=1&amp;y=2\">Link</a></p>\n\nDiagram\nSkip"),
    ({'form_fields': True},
     "Alpha beta gamma\ndelta\n\n x\n  y\n  1. First\n    • Nested\n  2. Second\n\nLink\n\nDiagram [ Find ] [ Note ]\nSkip"),
    ({'skip_elements': ['.skip-me']},
     "Alpha beta gamma\ndelta\n\n x\n  y\n  1. First\n    • Nested\n  2. Second\n\nLink\n\nDiagram"),
])
def test_synthetic_legacy_edge_cases(options, expected):
    actual = extract_plain_text(SYNTHETIC_EDGE_CASE_HTML, **options)
    assert actual.encode('utf-8') == expected.encode('utf-8')


def test_hidden_content_option():
    document = '<main><p>Article text</p><div class="hidden">More text</div></main>'
    assert extract_plain_text(document, main_content=True, hidden_elements=True) == \
           'Article text\n\nMore text'


def test_native_extraction_module():
    import importlib
    import inspect
    import resiliparse
    from resiliparse.parse.html.tree import HTMLTree as NativeHTMLTree

    package = importlib.import_module('resiliparse.extract')
    module = importlib.import_module('resiliparse.extract.html2text')
    assert resiliparse.extract is package
    assert package.html2text is module
    assert package.__all__ == []
    assert module.__all__ == ['extract_plain_text']
    assert inspect.isbuiltin(module.extract_plain_text)
    assert extract_plain_text(NativeHTMLTree.parse('<p>Native tree</p>')) == 'Native tree'


def test_inline_after_block():
    html = """<body>
<div>A</div>B

<div>C</div>

    D

<div>E</div><span>F</span>

<div>G</div><span>H</span>"""

    tree = HTMLTree.parse(html)
    assert extract_plain_text(tree, list_bullets=False) == "A\nB\nC\nD\nE\nF\nG\nH"


def test_pre_formatted():
    html = """<body>
    <ul>
        <li>A</li>
        <li>B<div>C</div>
            
        D   <p>E</p>
        <li>
                <pre>F
    G
        H
J</pre>
        </li>
    </ul>
    <textarea>K
        L
    </textarea>
    </body>"""

    tree = HTMLTree.parse(html)
    expected_without_bullets = """\
  A
  B
  C
  D

  E

  F
    G
        H
J"""

    expected_with_bullets = """\
  \u2022 A
  \u2022 B
    C
    D

    E

  \u2022 F
    G
        H
J"""

    expected_html_without_bullets = """\
A
B
C
D

<p>E</p>

<pre>F
    G
        H
J</pre>"""

    expected_html_with_bullets = """\
<ul>
  <li>A</li>
  <li>B
C
D

  <p>E  </p></li>
  <li> <pre>F
    G
        H
J</pre></li>
</ul>"""

    expected_textarea = """
[ K
        L
    ]"""

    assert extract_plain_text(tree, list_bullets=False) == expected_without_bullets
    assert extract_plain_text(tree, list_bullets=True) == expected_with_bullets

    assert extract_plain_text(tree, list_bullets=False, form_fields=True) == \
           expected_without_bullets + expected_textarea
    assert extract_plain_text(tree, list_bullets=True, form_fields=True) == \
           expected_with_bullets + expected_textarea

    assert extract_plain_text(tree, list_bullets=True, preserve_formatting='minimal_html') == expected_html_with_bullets
    assert extract_plain_text(tree, list_bullets=False,
                              preserve_formatting='minimal_html') == expected_html_without_bullets

    assert extract_plain_text(tree, list_bullets=True, preserve_formatting='minimal_html', form_fields=True) == \
           expected_html_with_bullets + expected_textarea
    assert extract_plain_text(tree, list_bullets=False, preserve_formatting='minimal_html', form_fields=True) == \
           expected_html_without_bullets + expected_textarea


def test_ordered_list():
    html = """<body>
<ul>
    <li>A</li>
    <li>B
        <ol>
            <li>C</li>
            <li>D
                <ol>
                    <li>E</li>
                    <li>F</li>
                </ol>
            </li>
            <li>G
                <ol>
                    <li>H</li>
                    <li>I</li>
                </ol>
            </li>
        </ol>
    </li>
</ul>
<ol>
    <li>J</li>
</ol>
</body>"""

    tree = HTMLTree.parse(html)
    assert extract_plain_text(tree, list_bullets=False) == """\
  A
  B
    C
    D
      E
      F
    G
      H
      I
  J"""

    assert extract_plain_text(tree, list_bullets=True) == """\
  \u2022 A
  \u2022 B
    1. C
    2. D
      1. E
      2. F
    3. G
      1. H
      2. I
  1. J"""

    assert extract_plain_text(tree, list_bullets=True, preserve_formatting='minimal_html') == """\
<ul>
  <li>A</li>
  <li>B
  <ol>
    <li>C</li>
    <li>D
    <ol>
      <li>E</li>
      <li>F</li>
    </ol></li>
    <li>G
    <ol>
      <li>H</li>
      <li>I</li>
    </ol></li>
  </ol></li>
</ul>
<ol>
  <li>J</li>
</ol>"""

    assert extract_plain_text(tree, list_bullets=False, preserve_formatting='minimal_html') == \
           "A\nB\nC\nD\nE\nF\nG\nH\nI\nJ"


def test_empty_list_items():
    html = """<body>
    <ul>
        <li>A</li>
        <li><button></button></li>
        <li><button>abc</button></li>
        <li></li>
        <li></li>
        <li>B</li>
        <li><button></button></li>
        <li>    </li>
    </ul>
    </body>"""

    assert extract_plain_text(html, list_bullets=False) == \
           '  A\n  B'
    assert extract_plain_text(html, list_bullets=True) == \
           '  \u2022 A\n  \u2022 B'


def test_html_escaping():
    html = """\
<h1>Hello World</h1>
<p><a href="https://example.com/?foo=bar&amp;bar=baz">link</a></p>
<pre>
Some code
&lt;html&gt;&amp;
<p>foo</p>
</pre>
&lt;html&gt;
<h2>&lt;html&gt;&amp;</h2>
<ul>
    <li>&lt;html&gt;&amp;</li>
</ul>
<textarea>&lt;html&gt;&amp;</textarea>"""

    expected = """\
<h1>Hello World</h1>

<p>{link}</p>

<pre>Some code
&lt;html&gt;&amp;
<p>foo</p>
</pre>
&lt;html&gt;

<h2>&lt;html&gt;&amp;</h2>

<ul>
  <li>&lt;html&gt;&amp;</li>
</ul>{textarea}"""

    link_expected = 'link'
    textarea_expected = ''
    assert extract_plain_text(html, preserve_formatting='minimal_html') == \
           expected.format(link=link_expected, textarea=textarea_expected)

    link_expected = '<a href="https://example.com/?foo=bar&amp;bar=baz">link</a>'
    textarea_expected = '\n[ &lt;html&gt;&amp; ]'
    assert extract_plain_text(html, preserve_formatting='minimal_html', links=True, form_fields=True) == \
           expected.format(link=link_expected, textarea=textarea_expected)

    expected = """\
Hello World

{link}

Some code
<html>&

foo
<html>

<html>&

  • <html>&
[ <html>& ]"""

    link_expected = 'link'
    assert extract_plain_text(html, preserve_formatting=True, links=False, form_fields=True) == \
           expected.format(link=link_expected)

    link_expected = 'link (https://example.com/?foo=bar&bar=baz)'
    assert extract_plain_text(html, preserve_formatting=True, links=True, form_fields=True) == \
           expected.format(link=link_expected)

    assert extract_plain_text(html, preserve_formatting=False, links=True, form_fields=True) == \
           "Hello World link (https://example.com/?foo=bar&bar=baz) Some code <html>& foo <html> <html>& <html>& [ <html>& ]"


def test_margin_collapsing():
    html = """\
<p>Hello
World</p>

<p>Hello<br>World<br><br><br><br></p>

<p>Hello<br>World<br><br><br><br></p>

<div>Hello World</div>"""

    assert extract_plain_text(html, preserve_formatting=True) == """\
Hello World

Hello\nWorld\n\n\n
Hello\nWorld\n\n\n
Hello World"""

    assert extract_plain_text(html, preserve_formatting='minimal_html') == """\
<p>Hello World</p>

<p>Hello<br>
World<br><br><br><br></p>

<p>Hello<br>
World<br><br><br><br></p>

Hello World"""


def test_real_word_data():
    # Cannot really compare expected outputs here, so only test that we have no crashes or anything
    i = 0
    for i, rec in enumerate(ArchiveIterator(open(os.path.join(DATA_DIR, 'warcfile.warc'), 'rb'),
                                            parse_http=True, record_types=WarcRecordType.response)):
        data = rec.reader.read()
        tree = HTMLTree.parse_from_bytes(data, rec.http_charset or 'utf-8')
        all = extract_plain_text(tree)
        assert all
        main_content = extract_plain_text(tree, main_content=True)
        assert main_content
        assert len(all) >= len(main_content)
    assert i > 0


def test_empty_formatting_string_is_false():
    # Both accepted Python argument types must select the same native formatting mode.
    markup = '<p>first</p><p>second</p>'
    assert extract_plain_text(markup, preserve_formatting='') == extract_plain_text(markup, preserve_formatting=False)
