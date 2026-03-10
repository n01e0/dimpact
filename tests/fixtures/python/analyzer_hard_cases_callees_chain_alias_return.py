# R53-2 fixture: extracted from python-heavy real-corpus pattern.
#
# Intent:
# - linear callee chain
# - alias/temporary assignment before return
# - stable names for comparing callees-direction FN/FP against strict-LSP oracle


def f00():
    v = 0
    return v


def f01():
    v = f00()
    inc = 1
    return v + inc


def f02():
    v = f01()
    inc = 1
    return v + inc


def f03():
    v = f02()
    inc = 1
    return v + inc


def f04():
    v = f03()
    inc = 1
    return v + inc


def f05():
    v = f04()
    inc = 1
    return v + inc


def f06():
    v = f05()
    inc = 1
    return v + inc


def f07():
    v = f06()
    inc = 1
    return v + inc


def f08():
    v = f07()
    inc = 1
    return v + inc


def f09():
    v = f08()
    inc = 1
    return v + inc


def f10():
    return f09() + 1


def entry():
    return f10()
