import importlib as il
from importlib import import_module as imod
from builtins import __import__ as dyn_import
from functools import wraps


class NormalizeDescriptor:
    def __get__(self, obj, objtype=None):
        def normalize(v):
            return v.strip()

        return normalize


def traced(fn):
    @wraps(fn)
    def wrapper(*args, **kwargs):
        return fn(*args, **kwargs)

    return wrapper


class DynamicResolverCombo:
    normalizer = NormalizeDescriptor()

    def __getattribute__(self, name):
        return super().__getattribute__(name)

    def __getattr__(self, name):
        if name == "dyn_handler":
            return lambda payload: payload.upper()
        raise AttributeError(name)

    @traced
    def run(self, name, payload):
        target = ".plugins.runtime"
        mod_a = il.import_module(target)

        common_mod = "pkg.plugins.common"
        mod_b = imod(common_mod)

        mod_c = dyn_import(f".ext.{name}")

        setter = "bound_handler"
        setattr(self, setter, lambda v: v.lower())
        handler = getattr(self, "bound_handler")

        via_dyn = getattr(self, "dyn_handler")(payload)
        cleaned = self.normalizer(via_dyn)

        out_a = mod_a.build(payload).handle()
        out_b = mod_b.make(payload).process()
        out_c = mod_c.create(payload).run()
        return handler(cleaned) + out_a + out_b + out_c
