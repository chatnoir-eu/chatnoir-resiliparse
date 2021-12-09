import os

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
    assert extract_plain_text(tree, alt_texts=False, preserve_formatting=False) == \
           "Nav 1 Nav 2 Nav 3 foo bar baz bar Copyright (C) 2021 Foo Bar"
    assert extract_plain_text(tree, alt_texts=False, list_bullets=False) == \
           "  Nav 1\n  Nav 2\n\n    Nav 3\n\nfoo bar\n\nbaz\nbar\n\nCopyright (C) 2021 Foo Bar"
    assert extract_plain_text(tree, alt_texts=False) == \
        "  \u2022 Nav 1\n  \u2022 Nav 2\n\n    \u2022 Nav 3\n\nfoo bar\n\nbaz\nbar\n\nCopyright (C) 2021 Foo Bar"


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
    assert extract_plain_text(tree, list_bullets=False) == expected_without_bullets

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
    assert extract_plain_text(tree, list_bullets=True) == expected_with_bullets

    expected_textarea = """
[ K
        L
     ]"""
    assert extract_plain_text(tree, list_bullets=False, form_fields=True) == \
           expected_without_bullets + expected_textarea
    assert extract_plain_text(tree, list_bullets=True, form_fields=True) == \
           expected_with_bullets + expected_textarea


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

    tree = HTMLTree.parse(html)
    assert extract_plain_text(tree, list_bullets=False) == \
           '  A\n  B'
    assert extract_plain_text(tree, list_bullets=True) == \
           '  \u2022 A\n  \u2022 B'


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
