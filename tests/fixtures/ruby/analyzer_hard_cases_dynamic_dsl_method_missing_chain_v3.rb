class DynamicRouteProxy
  def initialize(host)
    @host = host
  end

  def method_missing(name, *args)
    token = name.to_s
    if token.start_with?("via_")
      route = "route_#{token.delete_prefix("via_")}".to_sym
      return @host.public_send(route, *args)
    end

    super
  end

  def respond_to_missing?(name, include_private = false)
    name.to_s.start_with?("via_") || super
  end
end

class DynamicDslMethodMissingChainV3
  ALIAS_TARGETS = {
    "create" => :dispatch_created,
    "archive" => "dispatch_archived",
  }.freeze

  ROUTE_TARGETS = {
    "created" => :invoke_created,
    "archived" => "invoke_archived",
  }.freeze

  def deliver_created(payload)
    payload.to_s
  end

  def deliver_archived(payload)
    payload.to_s
  end

  def deliver_missing(payload)
    payload.to_s
  end

  alias_method :dispatch_created, :deliver_created
  alias_method "dispatch_archived", :deliver_archived

  module_eval do
    define_method(:invoke_created) do |payload|
      target = ALIAS_TARGETS["create"]
      send(target, payload)
    end

    define_method("invoke_archived") do |payload|
      target = ALIAS_TARGETS["archive"]
      public_send(target, payload)
    end
  end

  def proxy
    DynamicRouteProxy.new(self)
  end

  def method_missing(name, *args)
    key = name.to_s
    if key.start_with?("route_")
      action = key.delete_prefix("route_")
      target = ROUTE_TARGETS[action] || :deliver_missing
      return public_send(target, *args)
    end

    super
  end

  def respond_to_missing?(name, include_private = false)
    name.to_s.start_with?("route_") || super
  end

  def run
    proxy.via_created("alpha")
    proxy.via_archived("beta")
    route_unknown("gamma")
  end
end
