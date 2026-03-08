class DynamicAccessor:
    def __init__(self):
        setattr(self, "dyn_value", 1)

    def __getattr__(self, name):
        if name == "dyn_method":
            return lambda payload: payload.strip()
        raise AttributeError(name)

    def execute(self, payload):
        setattr(self, "bound_handler", lambda v: v.lower())
        handler = getattr(self, "bound_handler")
        direct = getattr(self, "dyn_method")(payload)
        via_name = getattr(self, "dyn_value")
        return handler(direct) + str(via_name)
