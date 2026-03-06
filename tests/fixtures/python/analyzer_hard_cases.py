import importlib
from importlib import import_module as imod
from .plugins import loader as local_loader
from .plugins import *


class DynamicRunner:
    def run(self, name, event):
        mod = importlib.import_module(f"plugins.{name}")
        handler = getattr(mod, "build")(event)
        extra = imod("plugins.extra")
        local_loader.load(name)
        return handler.handle(event) + extra.process(event)
