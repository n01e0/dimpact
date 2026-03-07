import importlib as il
from importlib import import_module as imod
from .plugins import loader as loader_mod
from .plugins import *


class DynamicRunner:
    def run(self, name, event):
        plugin = imod(f"plugins.{name}")
        via_alias = il.import_module("plugins.extra")
        handler = getattr(plugin, "build")(event)
        loader_mod.load(name)
        return handler.handle(event) + via_alias.process(event)
