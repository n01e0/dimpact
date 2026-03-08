from functools import wraps


class TrimDescriptor:
    def __get__(self, obj, objtype=None):
        def normalize(v):
            return v.strip()

        return normalize


def audited(tag):
    def deco(fn):
        @wraps(fn)
        def wrapper(*args, **kwargs):
            return fn(*args, **kwargs)

        return wrapper

    return deco


def traced(fn):
    @wraps(fn)
    def inner(*args, **kwargs):
        return fn(*args, **kwargs)

    return inner


class Pipeline:
    trim = TrimDescriptor()

    def __init__(self, sink):
        self.sink = sink

    @audited("dp70")
    @traced
    def run(self, value):
        cleaned = self.trim(value)
        return self.sink.client.emit(cleaned.lower())
