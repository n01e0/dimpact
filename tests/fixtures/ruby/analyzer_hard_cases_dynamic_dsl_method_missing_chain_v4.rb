class RoutedEventProxy
  def initialize(target, recorder)
    @target = target
    @recorder = recorder
  end

  def method_missing(name, *args)
    token = name.to_s
    if token.start_with?("decorate_")
      @recorder.record(token)
      route = "route_#{token.delete_prefix("decorate_")}".to_sym
      return @target.public_send(route, *args)
    end

    super
  end

  def respond_to_missing?(name, include_private = false)
    name.to_s.start_with?("decorate_") || super
  end
end

class AuditRecorder
  def record(event_name)
    event_name
  end
end

class DynamicDslMethodMissingChainV4
  ALIAS_TARGETS = {
    created: :dispatch_created,
    "cancelled" => "dispatch_cancelled",
  }.freeze

  ROUTE_TARGETS = {
    "created" => :emit_created,
    "cancelled" => "emit_cancelled",
  }.freeze

  def persist_created(payload)
    payload.to_s
  end

  def persist_cancelled(payload)
    payload.to_s
  end

  def persist_unknown(payload)
    payload.to_s
  end

  alias_method :dispatch_created, :persist_created
  alias_method "dispatch_cancelled", :persist_cancelled

  module_eval do
    define_method(:emit_created) do |payload|
      target = ALIAS_TARGETS[:created]
      send(target, payload)
    end

    define_method("emit_cancelled") do |payload|
      route_key = "cancelled"
      target = ALIAS_TARGETS[route_key]
      public_send(target, payload)
    end
  end

  def proxy
    RoutedEventProxy.new(self, AuditRecorder.new)
  end

  def method_missing(name, *args)
    token = name.to_s
    if token.start_with?("route_")
      action = token.delete_prefix("route_")
      target = ROUTE_TARGETS[action] || :persist_unknown
      return public_send(target, *args)
    end

    super
  end

  def respond_to_missing?(name, include_private = false)
    name.to_s.start_with?("route_") || super
  end

  def run
    proxy.decorate_created("alpha")
    proxy.decorate_cancelled("beta")

    route_name = "route_created"
    public_send(route_name, "gamma")

    route_unknown("delta")
  end
end
