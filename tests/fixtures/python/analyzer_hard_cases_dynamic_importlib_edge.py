import importlib
from importlib import import_module as imod

from .plugins import loader as loader_mod
from ..core import registry


class DynamicImportEdge:
    def run(self, name, payload):
        mod1 = importlib.import_module(f".plugins.{name}", package=__package__)
        out1 = getattr(mod1, "build")(payload).handle()

        mod2 = imod("pkg.plugins.common")
        out2 = getattr(mod2, "make")(payload).process()

        local = loader_mod.load(name)
        reg = registry.lookup(name)
        return local.handle(out1) + reg.process(out2)
