from typing import Protocol, runtime_checkable


class MetaRouter(type):
    def __getattr__(cls, name):
        if name == "build":
            return lambda payload: cls._build(payload)
        raise AttributeError(name)


@runtime_checkable
class SupportsRun(Protocol):
    def run(self, payload: str) -> str: ...


class RuntimePatch:
    def run(self, payload: str) -> str:
        return payload


def patched_run(self, payload: str) -> str:
    return payload.strip().upper()


def install_patch() -> None:
    attr_name = "run"
    setattr(RuntimePatch, attr_name, patched_run)


class Service(RuntimePatch, metaclass=MetaRouter):
    @classmethod
    def _build(cls, payload: str) -> str:
        return cls().run(payload)


def execute(obj: SupportsRun, payload: str) -> str:
    install_patch()
    if isinstance(obj, SupportsRun):
        method_name = "run"
        return getattr(obj, method_name)(payload)
    return Service.build(payload)
