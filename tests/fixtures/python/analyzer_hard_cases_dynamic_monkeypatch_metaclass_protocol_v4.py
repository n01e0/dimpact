from functools import wraps
from typing import Protocol, runtime_checkable


def instrument(label: str):
    def decorate(fn):
        @wraps(fn)
        def wrapped(*args, **kwargs):
            return fn(*args, **kwargs)

        return wrapped

    return decorate


class MetaFactory(type):
    def __getattr__(cls, name: str):
        if name.startswith("build_"):
            route = name.removeprefix("build_")
            return lambda payload: cls._make(route, payload)
        raise AttributeError(name)


@runtime_checkable
class SupportsProcess(Protocol):
    def process(self, payload: str) -> str: ...


class Worker:
    def process(self, payload: str) -> str:
        return payload


def patched_process(self, payload: str) -> str:
    return payload.strip().lower()


def install_patch() -> None:
    method_name = "process"
    setattr(Worker, method_name, patched_process)


class ServiceFactory(Worker, metaclass=MetaFactory):
    @classmethod
    @instrument("factory")
    def _make(cls, route: str, payload: str) -> str:
        instance = cls()
        method_name = "process"
        return f"{route}:{getattr(instance, method_name)(payload)}"


def invoke_protocol(target: SupportsProcess, payload: str) -> str:
    method_name = "process"
    return getattr(target, method_name)(payload)


def invoke_class(payload: str) -> str:
    factory_name = "build_worker"
    return getattr(ServiceFactory, factory_name)(payload)


@instrument("dispatch")
def execute(target: SupportsProcess, payload: str) -> str:
    install_patch()
    if isinstance(target, SupportsProcess):
        return invoke_protocol(target, payload)
    return invoke_class(payload)
