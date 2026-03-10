from functools import wraps
from typing import Protocol, runtime_checkable


def audit_wrapper(label: str):
    def decorate(fn):
        @wraps(fn)
        def wrapped(*args, **kwargs):
            return fn(*args, **kwargs)

        return wrapped

    return decorate


class MetaBridge(type):
    def __getattr__(cls, name: str):
        if name.startswith("build_"):
            route = name.removeprefix("build_")
            return lambda payload: cls._build(route, payload)
        raise AttributeError(name)


@runtime_checkable
class SupportsDispatch(Protocol):
    def dispatch(self, payload: str) -> str: ...


class Handler:
    def dispatch(self, payload: str) -> str:
        return payload


def patched_dispatch(self, payload: str) -> str:
    return payload.strip().upper()


def install_patch() -> None:
    attr_name = "dispatch"
    setattr(Handler, attr_name, patched_dispatch)


class Router(Handler, metaclass=MetaBridge):
    strategy_name = "dispatch"

    @classmethod
    @audit_wrapper("build")
    def _build(cls, route: str, payload: str) -> str:
        method_name = cls.strategy_name
        return f"{route}:{getattr(cls(), method_name)(payload)}"


def invoke_protocol(target: SupportsDispatch, payload: str) -> str:
    method_name = "dispatch"
    return getattr(target, method_name)(payload)


def invoke_class(payload: str) -> str:
    class_attr = "build_service"
    return getattr(Router, class_attr)(payload)


def execute(target: SupportsDispatch, payload: str) -> str:
    install_patch()
    if isinstance(target, SupportsDispatch):
        return invoke_protocol(target, payload)
    return invoke_class(payload)
