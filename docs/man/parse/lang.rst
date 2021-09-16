.. _parse-lang-manual:

Language Tools
==============

Resiliparse language tools.

.. _parse-fast-langdetect-chunked:

Fast Language Detection
-----------------------

Resiliparse has a very fast n-gram-based language detector for 110 languages that can be used for fast bulk tagging of many input texts. The model is extremely simple and runs in linear time with only a single pass over the text, making it much faster than other language detection tools for Python. The speed obviously comes at the cost of accuracy (about 80-85% for tweet-sized texts, better performance for longer inputs), so if precision is important, you should probably use a more sophisticated model such as `FastText <https://fasttext.cc/blog/2017/10/02/blog-post.html>`_ (though Resiliparse's language detector can still be useful for pre-filtering).

.. code-block:: python

  from resiliparse.parse.lang import detect_fast as d

  print(d('''Call me Ishmael. Some years ago—never mind how long precisely—having
             little or no money in my purse, and nothing particular to interest me
             on shore, I thought I would sail about a little and see the watery part
             of the world.'''))
  # >>> ('en', 431)

  print(d('''Als Gregor Samsa eines Morgens aus unruhigen Träumen erwachte, fand er
             sich in seinem Bett zu einem ungeheueren Ungeziefer verwandelt.'''))
  # >>> ('de', 603)

  print(d('''Le 24 février 1815, la vigie de Notre-Dame de la Garde signala le
             trois-mâts le Pharaon, venant de Smyrne, Trieste et Naples.'''))
  # >>> ('fr', 608)

:func:`~.parse.lang.detect_fast` returns a tuple with the detected language and its `out-of-place measure`, a rank-order value indicating how much the text's n-gram ranks differ from the closest pre-trained language profile. The lower the value, the more accurate the prediction probably is. Values above 1200 are most likely false results. Longer input texts usually lead to much lower rank values and hence more accurate results.

Instead of only a single result, the language detector can also return a sorted list of possible language matches if you want to analyze and weight them further:

.. code-block:: python

  print(d('''To Sherlock Holmes she is always the woman. I have seldom heard him
             mention her under any other name. In his eyes she eclipses and
             predominates the whole of her sex. It was not that he felt any emotion
             akin to love for Irene Adler.''', n_results=5))
  # >>> [('en', 431), ('da', 481), ('no', 487), ('af', 492), ('li', 493)]

If you know your text is from one of several candidate languages, you can restrict the detection to those languages in order to improve the precision (and also slightly increase the performance by reducing the number of comparisons):

.. code-block:: python

  print(d('''En un lugar de la Mancha, de cuyo nombre no quiero acordarme, no ha mucho
             tiempo que vivía un hidalgo de los de lanza en astillero, adarga antigua,
             rocín flaco y galgo corredor.''',
             langs=['it', 'es', 'ca', 'en', 'de'], n_results=3))
  # >>> [('es', 542), ('it', 595), ('ca', 612)]

On an average webpage, Resiliparse's fast language detector is about 5x as fast as FastText (with the large model) and even 45x as fast as `langid <https://github.com/saffsd/langid.py>`_:

::

  Language detection (10000 rounds):
  Resiliparse: 3.3s
  FastText: 18.7s
  Langid: 150.81s

Resiliparse's performance advantage comes mostly from the fact that the language detector does not need to tokenize the text or build a vocabulary map at all, which makes it very low-latency, independent of the vocabulary size, and guarantees a fixed memory ceiling.

Supported languages are:

  af, an, ar, as, az, ba, be, bg, bn, bo, br, bs, ca, ce, cs, cv, cy, da, de, dv, el, en, eo, es, et, eu, fa, fi, fo, fr, fy, ga, gd, gl, gu, ha, he, hi, hr, hu, hy, ia, id, io, is, it, ja, jv, ka, kk, km, kn, ko, ku, ky, la, lb, li, lt, lv, mg, mk, ml, mn, mr, ms, mt, my, ne, nl, nn, no, oc, or, pa, pl, ps, pt, rm, ro, ru, sa, sc, sd, sh, si, sk, sl, so, sq, sr, su, sv, sw, ta, te, tg, th, tk, tl, tr, tt, ug, uk, ur, uz, vi, vo, yi, zh
