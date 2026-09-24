#!/usr/bin/env python3
# 2026-09-24: dated module comment
import re

def f(x):
    s = "# DECOY"  # trailing comment
    t = '''
    # DECOY inside a triple-quoted string
    '''
    r = r"\" # DECOY still inside the raw string"  # after raw
    u = f"{x}#{x}"  # f-string
    return re.compile(r"#")  # noqa: E501
