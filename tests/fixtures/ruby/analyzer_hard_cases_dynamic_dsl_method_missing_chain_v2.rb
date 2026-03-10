module EventSink
  def emit_created(payload)
    payload.to_s
  end

  def emit_deleted(payload)
    payload.to_s
  end

  def emit_unknown(payload)
    payload.to_s
  end
end

class RouteProxy
  def initialize(host, prefix = [])
    @host = host
    @prefix = prefix
  end

  def method_missing(name, *args)
    token = name.to_s

    if token == "resource" && !args.empty?
      return RouteProxy.new(@host, @prefix + [args.first.to_s])
    end

    if token.end_with?("!")
      action = token.delete_suffix("!")
      return @host.route_bang(@prefix + [action], *args)
    end

    RouteProxy.new(@host, @prefix + [token])
  end

  def respond_to_missing?(_name, _include_private = false)
    true
  end
end

class DynamicDslMethodMissingChainV2
  include EventSink

  ACTION_MAP = {
    "create" => :emit_created,
    "delete" => "emit_deleted",
  }.freeze

  def dsl
    RouteProxy.new(self)
  end

  def route_bang(tokens, payload)
    dynamic = "route_#{tokens.last}".to_sym
    send(dynamic, payload)
  end

  def method_missing(name, *args)
    key = name.to_s
    if key.start_with?("route_")
      action = key.delete_prefix("route_")
      return dynamic_route(action, *args)
    end

    super
  end

  def respond_to_missing?(name, include_private = false)
    name.to_s.start_with?("route_") || super
  end

  def dynamic_route(action, payload)
    target = ACTION_MAP[action] || :emit_unknown
    public_send(target, payload)
  end

  def run
    dsl.api.resource(:users).create!("alice")
    dsl.admin.resource("teams").delete!("core")
    route_create("manual")
    route_unknown("noop")
  end
end
