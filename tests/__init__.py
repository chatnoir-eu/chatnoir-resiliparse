import os
import sys

src_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))
sys.path.extend([
    os.path.join(src_dir, 'resiliparse'),
    os.path.join(src_dir, 'fastwarc')
])
