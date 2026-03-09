module PrependHook
  def mixed_from_prepend
    :ok
  end
end

module IncludedApi
  def included_api
    :ok
  end
end

module ClassApi
  def class_api
    :ok
  end
end

class DynamicResolverCombo
  prepend PrependHook
  include IncludedApi
  extend ClassApi

  def base_action
    :ok
  end

  alias_method :base_alias, :base_action

  define_method("base_defined") do
    base_alias
  end

  def method_missing(name, *args)
    return :dynamic if name.to_s.start_with?("dyn_")
    super
  end

  def respond_to_missing?(name, include_private = false)
    name.to_s.start_with?("dyn_") || super
  end

  def execute
    dyn_name = "base_defined"
    public_send(dyn_name)
    send(:base_alias)
    included_api
    mixed_from_prepend
    self.class_api
    dyn_routed
  end
end
