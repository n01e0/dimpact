# R53-2 fixture: ruby counterpart for real-corpus call-chain shape.
#
# Intent:
# - linear callee chain
# - alias/temporary assignment before return
# - reusable once strict ruby-lsp oracle lane is stable in environment

def f00
  v = 0
  v
end

def f01
  v = f00
  inc = 1
  v + inc
end

def f02
  v = f01
  inc = 1
  v + inc
end

def f03
  v = f02
  inc = 1
  v + inc
end

def f04
  v = f03
  inc = 1
  v + inc
end

def f05
  v = f04
  inc = 1
  v + inc
end

def f06
  v = f05
  inc = 1
  v + inc
end

def f07
  v = f06
  inc = 1
  v + inc
end

def f08
  v = f07
  inc = 1
  v + inc
end

def f09
  v = f08
  inc = 1
  v + inc
end

def f10
  f09 + 1
end

def entry
  f10
end
