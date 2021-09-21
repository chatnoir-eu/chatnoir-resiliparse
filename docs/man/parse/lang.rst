.. _parse-lang-manual:

Language Tools
==============

Resiliparse language tools.

.. _parse-fast-langdetect-chunked:

Fast Language Detection
-----------------------

Resiliparse has a very fast n-gram-based language detector for 101 languages that can be used for fast bulk tagging of many input texts. The model is extremely simple and runs in linear time with only a single pass over the text, making it much faster than other language detection tools.

.. code-block:: python

  from resiliparse.parse.lang import detect_fast as d

  print(d('''Call me Ishmael. Some years ago—never mind how long precisely—having
             little or no money in my purse, and nothing particular to interest me
             on shore, I thought I would sail about a little and see the watery part
             of the world.'''))
  # >>> ('en', 431)

  print(d('''Als Gregor Samsa eines Morgens aus unruhigen Träumen erwachte, fand er
             sich in seinem Bett zu einem ungeheueren Ungeziefer verwandelt.'''))
  # >>> ('de', 609)

  print(d('''Le 24 février 1815, la vigie de Notre-Dame de la Garde signala le
             trois-mâts le Pharaon, venant de Smyrne, Trieste et Naples.'''))
  # >>> ('fr', 612)

:func:`~.parse.lang.detect_fast` returns a tuple with the detected language and its `out-of-place measure`, a rank-order value indicating how much the text's n-gram ranks differ from the closest pre-trained language profile. The lower the value, the more accurate the prediction probably is. Values above 1200 are most likely false results. Longer input texts usually lead to much lower rank values and hence more accurate results.

Instead of only a single result, the language detector can also return a sorted list of possible language matches if you want to analyze and weight them further:

.. code-block:: python

  print(d('''To Sherlock Holmes she is always the woman. I have seldom heard him
             mention her under any other name. In his eyes she eclipses and
             predominates the whole of her sex. It was not that he felt any emotion
             akin to love for Irene Adler.''', n_results=5))
  # >>> [('en', 431), ('da', 461), ('no', 467), ('af', 472), ('fy', 494)]

If you know your text is from one of several candidate languages, you can restrict the detection to those languages in order to improve the precision (and also slightly increase the performance by reducing the number of comparisons):

.. code-block:: python

  print(d('''En un lugar de la Mancha, de cuyo nombre no quiero acordarme, no ha mucho
             tiempo que vivía un hidalgo de los de lanza en astillero, adarga antigua,
             rocín flaco y galgo corredor.''',
             langs=['it', 'es', 'ca', 'en', 'de'], n_results=3))
  # >>> [('es', 541), ('it', 588), ('ca', 592)]


.. _parse-fast-langdetect-performance:

Benchmarks
^^^^^^^^^^
On inputs the size of an average webpage, Resiliparse's fast language detector is about 2-5x as fast as `FastText <https://fasttext.cc/blog/2017/10/02/blog-post.html>`_ (depends on FastText's convergence speed) and even 32x as fast as `langid <https://github.com/saffsd/langid.py>`_:

::

  Benchmarking language detectors (10,000 rounds):
  Resiliparse: 3.0s
  FastText:    6.4s
  Langid:      98.0s

Resiliparse's performance advantage comes mostly from the fact that the language detector does not need to tokenize the text or build a vocabulary map at all, which makes it very low-latency, independent of the vocabulary size, and guarantees a fixed memory ceiling and linear runtime complexity.

The enormous speed obviously comes at the cost of some accuracy compared to other state-of-the art language detection models. For most languages, you can expect an F1 of *90-99%+* on inputs of at least one paragraph or longer (*96%* macro average accuracy over all supported languages). Some extremely similar languages (such as Danish and Norwegian) tend to perform worse than that. If the input text is extremely short, you will also see a significant performance drop. On single sentences or tweets, about *75-85%* F1 are realistic for languages with Latin alphabets and *85-99%* for more idiosyncratic writing systems. If you need higher accuracy than that (particularly on short text snippets), you may want to use a more sophisticated model, such as FastText. You can also combine both models and use Resiliparse with a (very) conservative out-of-place rank cutoff for high-precision/low-recall pre-filtering and then use FastText for samples above that cutoff threshold.

Supported Languages
^^^^^^^^^^^^^^^^^^^
The following 101 languages are supported:

  af, ar, as, az, ba, be, bg, bn, bo, br, ca, ce, cs, cv, cy, da, de, dv, el, en, eo, es, et, eu, fa, fi, fo, fr, fy, ga, gd, gl, gu, ha, he, hi, hr, hu, hy, id, io, is, it, ja, jv, ka, kk, km, kn, ko, ku, ky, la, lb, lt, lv, mg, mk, ml, mn, mr, mt, my, ne, nl, no, or, pa, pl, ps, pt, rm, ro, ru, sa, sc, sd, si, sk, sl, so, sq, sr, sv, sw, ta, te, tg, th, tk, tl, tr, tt, ug, uk, ur, uz, vi, vo, yi, zh
