module AuditHook
  def track(payload)
    payload.to_s
  end
end

class DynamicDslMethodMissingChain
  include AuditHook

  HANDLER_MAP = {
    "dsl_create" => :handle_create,
    "dsl_update" => "handle_update",
  }.freeze

  def method_missing(name, *args)
    key = name.to_s
    if HANDLER_MAP.key?(key)
      dynamic_dispatch(key, *args)
    else
      super
    end
  end

  def respond_to_missing?(name, include_private = false)
    HANDLER_MAP.key?(name.to_s) || super
  end

  def dynamic_dispatch(key, *args)
    target = HANDLER_MAP[key]
    public_send(target, *args)
  end

  def handle_create(payload)
    track(payload)
  end

  def handle_update(payload)
    track(payload)
  end

  def replay(payload)
    routed = :handle_update
    send(routed, payload)
  end

  def run
    dsl_create("a")
    dsl_unknown("b")
    replay("c")
  end
end
