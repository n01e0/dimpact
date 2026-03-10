from typing import Protocol, runtime_checkable


class MetaFactory(type):
    def __getattr__(cls, name: str):
        if name.startswith("make_"):
            tag = name.removeprefix("make_")
            return lambda payload: cls._from_tag(tag, payload)
        raise AttributeError(name)


@runtime_checkable
class SupportsHandle(Protocol):
    def handle(self, payload: str) -> str: ...


class Worker:
    def handle(self, payload: str) -> str:
        return payload


class Engine(Worker, metaclass=MetaFactory):
    @classmethod
    def _from_tag(cls, tag: str, payload: str) -> str:
        method = "handle"
        value = getattr(cls(), method)(payload)
        return f"{tag}:{value}"


def patched_handle(self, payload: str) -> str:
    return payload.strip().lower()


def patch_worker() -> None:
    method_name = "handle"
    setattr(Worker, method_name, patched_handle)


def call_protocol(target: SupportsHandle, payload: str) -> str:
    dynamic_name = "handle"
    return getattr(target, dynamic_name)(payload)


def execute(target: SupportsHandle, payload: str) -> str:
    patch_worker()
    if isinstance(target, SupportsHandle):
        return call_protocol(target, payload)
    return Engine.make_service(payload)
