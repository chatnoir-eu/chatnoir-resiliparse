import os

import pytest
from fastwarc.warc import *
from resiliparse.parse.html import *
from resiliparse.extract.html2text import *

DATA_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', 'data'))

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
    for inp in [html, tree]:    # Run twice, once with string input, once with HTML tree
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


def test_skip_elements():
    # Script, style (also inside SVG), template and noscript elements are skipped by default
    html = '<p>a</p><script>s</script><style>s</style><svg><style>x</style></svg>' \
           '<template><p>t</p></template><noscript>n</noscript><p>b</p>'
    assert extract_plain_text(html) == 'a\n\nb'
    assert extract_plain_text('<p>a</p><noscript>n</noscript>', noscript=True) == 'a\n\nn'

    # Tag names (in any case) and other CSS selectors can be mixed
    html = '<nav>n</nav><p class="ads">ad</p><P>keep</P><ASIDE>aside</ASIDE>'
    assert extract_plain_text(html, skip_elements=['nav', '.ads', 'ASIDE']) == 'keep'


def test_main_content_extraction():
    assert extract_plain_text(tree, alt_texts=False, main_content=True) == \
           "foo\n\nbaz\nbar"
    assert extract_plain_text(tree, alt_texts=True, main_content=True) == \
           "foo\n\nbaz\nbar\n\nSome image"
    assert extract_plain_text(tree, alt_texts=False, main_content=True, form_fields=True) == \
           "foo\n\nbaz\nbar\n\n[ Some text ] [ Insert text ]"


def test_main_content_root():
    # A single element matching one of the main content root selectors becomes the extraction root
    other = '<div><p>Other</p></div>'
    assert extract_plain_text(other + '<div class="article-body"><p>Body</p></div>', main_content=True) == 'Body'
    assert extract_plain_text(other + '<div class="x ARTICLE-BODY"><p>Body</p></div>', main_content=True) == 'Body'
    assert extract_plain_text(other + '<section role="main"><p>Body</p></section>', main_content=True) == 'Body'
    assert extract_plain_text(other + '<section role="Main"><p>Body</p></section>', main_content=True) == \
           'Other\n\nBody'

    # The root is not used if there is more than one match (an element matching two selectors counts twice)
    html = other + '<div class="post-content"><p>Body</p></div><div class="post-content"><p>Body 2</p></div>'
    assert extract_plain_text(html, main_content=True) == 'Other\n\nBody\n\nBody 2'
    html = other + '<main class="main-content" role="main"><p>Body</p></main>'
    assert extract_plain_text(html, main_content=True) == 'Other\n\nBody'

    # The root itself is never skipped, even if it is a skipped element type
    html = other + '<noscript class="article-body"><p>Body</p></noscript>'
    assert extract_plain_text(html, main_content=True) == 'Body'


def test_main_content_link_clusters():
    def nested(html):
        return '<div><div><div><div>' + html + '</div></div></div></div><p>After</p>'

    # Lists are dropped if more than 20% of their text (with collapsed white space) is link text
    assert extract_plain_text(nested('<ul><li><a>aaaa</a></li><li>b' + ' ' * 50 + 'c</li></ul>'),
                              main_content=True) == 'After'
    assert extract_plain_text(nested('<ul><li><a>aaaa </a></li><li>  ' + 'b ' * 9 + '</li></ul>'),
                              main_content=True) == 'After'

    # Script text counts as text, nested links count twice
    assert extract_plain_text(nested('<ul><li><a>link</a>' + 't' * 10 + '<script>' + 'x' * 30 + '</script></li></ul>'),
                              main_content=True) == '  \u2022 linktttttttttt\n\nAfter'
    assert extract_plain_text(nested('<ul><li><a href="#">' + 'o' * 10 + '<svg><a>' + 'i' * 40 + '</a></svg></a>' +
                                     'w' * 300 + '</li></ul>'), main_content=True) == 'After'

    # Deeply nested divs are only checked up to 800 characters of text
    html = '<div>' * 7 + '<a>link</a> ' * 150 + '</div>' * 7 + '<p>After</p>'
    assert extract_plain_text(html, main_content=True) == 'After'
    html = '<div>' * 7 + '<a>link</a> ' * 200 + '</div>' * 7 + '<p>After</p>'
    assert extract_plain_text(html, main_content=True).startswith('link link')


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
    assert extract_plain_text(tree, list_bullets=False, preserve_formatting='minimal_html') == expected_html_without_bullets

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
