module BeforeHook
  def around_before
    :before
  end
end

module IncludedHook
  def from_included
    :included
  end
end

class MissingIncludePrepend
  prepend BeforeHook
  include IncludedHook

  def method_missing(name, *args)
    return :dynamic if name.to_s.start_with?("dyn_")
    super
  end

  def respond_to_missing?(name, include_private = false)
    name.to_s.start_with?("dyn_") || super
  end

  def execute
    dyn_alpha
    from_included
    around_before
  end
end
