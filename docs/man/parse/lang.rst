.. _parse-lang-manual:

Language Tools
==============

Resiliparse language tools.

.. _parse-fast-langdetect-chunked:

Fast Language Detection
-----------------------

Resiliparse has a very fast n-gram-based language detector for 17 languages that can be used for fast bulk tagging of many input texts. The model is extremely simple and runs in linear time, making it much faster than other language detection tools for Python. The speed obviously comes at the cost of accuracy (about 85-90% depending on the language and size of the input text), so if that is important, you should probably use a more sophisticated model such as `FastText <https://fasttext.cc/blog/2017/10/02/blog-post.html>`_ (though Resiliparse's language detector can still be useful for pre-filtering).

.. code-block:: python

  from resiliparse.parse.lang import detect_fast

  print(detect_fast('This is an average English text...'))
  # >>> ('en', 781)

  print(detect_fast('...aber fügen wir doch etwas auf Deutsch hinzu.'))
  # >> ('de', 655)

:func:`~.parse.lang.detect_fast` returns a tuple with the detected language and its `out-of-place measure`, a rank-order value indicating how much the text's n-gram ranks differ from the closest pre-trained language profile. The lower the value, the more accurate the prediction probably is. Values above 1000 are most likely false results. Longer input texts usually lead to much lower rank values and hence more accurate results.

On an average webpage, Resiliparse's fast language detector is about 4x as fast as FastText (with the large model) and even 32x as fast as `langid <https://github.com/saffsd/langid.py>`_:

::

  Language detection (10000 rounds):
  Resiliparse: 4.57s
  FastText: 21.38s
  Langid: 150.81s

Resiliparse's performance advantage comes mostly from the fact that the language detector does not need to tokenize the text or build a vocabulary map at all, which makes it very low-latency, independent of the vocabulary size, and guarantees a fixed memory ceiling.
