from functools import wraps as w


class UpperDescriptor:
    def __get__(self, obj, objtype=None):
        def inner(v):
            return v.upper()

        return inner


def traced(fn):
    @w(fn)
    def wrapper(*args, **kwargs):
        return fn(*args, **kwargs)

    return wrapper


class Service:
    normalizer = UpperDescriptor()

    def __init__(self, api):
        self.api = api

    @traced
    def process(self, value):
        cleaned = self.normalizer(value)
        return self.api.client.dispatcher.send(cleaned.strip().lower())
